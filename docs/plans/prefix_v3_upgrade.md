# Prefix v3 upgrade: compiler, source catalogue and WebUI

## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
Each real Skill has a canonical execution Recipe with a matching command.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](../../skills.md#preloadable-function-interface-binding-v3-target).
This is a binding target, not proof of implemented loader/store/runner support.
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


Status: implementation plan, not shipped functionality. Source and evidence review:
2026-10-09. This revision incorporates Sempai quality experiments, the reproduced
LMCache transfer failure and the v25 compiler/client protocol corrections. The v25
source and decoder checks are development evidence: no v25 prefix generation or
model-accuracy qualification has been published by those checks. This plan update
does not deploy code or change inference services.
The workspace package version inspected on 2026-10-12 is `1.7.0-rc.4`; release identity also needs a commit and
content manifest because version numbers alone do not identify the working tree.

### Implementation reconciliation — 2026-10-11

Read this plan together with [Sempai v3.5](sempai_v3.5.md) and
[Simplify connections v3.5](simplify_connections_v3.5.md). Their consolidation
work supplies shared infrastructure; prefix work must not create a second review
consumer, provider registry, connection builder, Monty transport, executor,
claim authority or universal recovery ledger. Similar record names do not make
generation jobs, invocation effects and provider accounting interchangeable.

| Responsibility | Reuse owner / current evidence | Prefix-specific work still required |
| --- | --- | --- |
| Workflow selection / execution | Installed/authored Monty catalogue, retained preload/export qualification and the global service | Qualify the generation Recipe and exact compiler adapter; do not start another VM or Rust workflow loop. |
| Component proposal retention / activation | Existing submission stores and [component deployment](../reborn/component-deployment.md); real operator/PG/global-Monty acceptance passed | Use these paths for executable proposals. Prefix generation/design publication is a distinct operation and is not implemented by component activation. |
| Review/accounting custody | Existing completed-turn delivery and [native reconciliation](../reborn/post-turn-review-reconciliation.md) | Preserve native release predicates. Sempai evidence/proposal/iteration conversion remains owned by sempai_v3.5; do not duplicate it in prefix jobs. |
| Model calls / messages / evidence | Existing gateway plus the reusable usages planned in sempai_v3.5 | Redesign uses shared message/evidence/model-call components with prefix-specific instructions supplied as pinned data. No separate design-provider transport or review sink. |
| Database / network / IPC | Instance-injected PgPool, existing network/egress services and Monty transport actor; consolidation owned by simplify_connections_v3.5 | Extend owning stores only for demonstrated prefix records. No per-profile pools, caller-local reconnect loops or parallel pipe readers. |
| Admission / events / retries | Existing turns/global-task admission, retained invocation records and durable event projection contracts | Store prefix stage/artifact metadata linked to actual runs/invocations. Observer reconnect reads status/cursors; it never resubmits effects. Generic crash resume remains unproven until its actual contract passes. |
| Operator settings / APIs | Existing RebornServicesApi, authenticated v2 ingress and revisioned settings owners | Extend Prefix settings rather than building another management surface or authority path. |

Completed infrastructure is reusable evidence for its exact scope, not a blanket
Gate A1 certificate. Phase B's offline source-package/compiler and scoped catalogue
export have focused acceptance below. The Phase C registry foundation lists all six
shipped profiles through the existing prefix API, while regeneration remains the
legacy `base-prompt` workflow. The compatibility scope store still has minimal
fallbacks; those are not permitted substitutes for a missing required production
prefix. Immutable generation retention and source/model metadata now have owning
stores and operator surfaces; generation jobs, prefix activation/resolver, qualified
model designs, runtime Prefix-Maker, source checking, Redesign and custom-profile
creation remain open. Metadata/import surfaces do not close their execution gates.

Continue Phase C from the existing registry and accepted Phase B adapter. Complete
immutable generation storage, pinned inputs and runtime compiler qualification
before enabling target-aware generation. Keep shared evidence/render/token-probe
functions rather than adding copied pipelines. This work never bypasses Gate A1.
Database exports come from the existing catalogue owner under one snapshot; local
source-package manifests remain build inputs, not another authoritative installed
catalogue or approval store.

## 1. Recipe-first workflow and intended result

Enhance the existing Prefix settings page to manage six seeded profiles and operator-created profiles:
`base-prompt`, `defensive`, `homeassistant`, `tomedo`, `sempai` and `prefix-maker`. Each has a Generate /
Regenerate action, independent build status, immutable generations, source quality
details and an explicit deployment/cache status. Replace the current base-prompt
row concatenation with the exact-source compiler approach already used by the
domain compilers. Preserve the large-prefix objective and retain the complete
source library even when one serving model cannot hold it all.

The operator workflow is a Recipe, not a new Rust workflow Tool:

```text
Generate / Regenerate
  -> admit durable generation job for a registered profile
  -> pin approved component selection and release-document source snapshot
  -> export immutable compiler input
  -> run the profile compiler in an isolated worker
  -> validate evidence, coverage, template and actual token geometry
  -> publish immutable generation; finish the build job

Activate generation (separate operation)
  -> validate provider/profile compatibility
  -> atomically select generation for new model requests
  -> optionally warm the selected provider
  -> verify exact injected tokens and measured cache reuse

Redesign / Create prefix (separate Tier-1 Recipes)
  -> obtain explicit confirmation of the provider-assisted design request
  -> refresh/review applicable content and architecture evidence; pin current snapshot
  -> pin Prefix-Maker, model-specific design/source evidence and provider/target metadata
  -> provider proposes a model-specific design or new profile draft
  -> retain proposal; validate contracts and compile a complete updated candidate generation
  -> show design/content diff and quality/capacity evidence
  -> operator accepts validated design and publishes its exact updated generation
  -> later Generate uses that model-specific revision; Activate remains separate
```

### 1.1 What one click must deliver

Generate is a durable, model-free build workflow, not direct invocation of a
developer script. For a selected registered target it produces a reproducible,
source-backed, capacity-checked artifact suitable for the declared consumer. For
Kohai/Sempai that means the required base-plus-domain/reviewer composite, not just
a parent reference requiring an undocumented second manual build. Orchestrate
missing/outdated parents and composite compilation as separate reusable Recipe
tasks under one visible operation; reuse eligible immutable parents. Pin a coherent
source/parent selection and retain child job references. Never secretly activate,
change a provider template, or replace an inference deployment on Generate.

Without a configured provider, Generate can still build a source generation against
registered local tokenizer/model artifacts. Label it Generated / target not selected;
it cannot promise serving readiness. Missing worker/artifacts, unreviewed source
changes and insufficient context have actionable blocked results. The UI must not
say “successful” when a subprocess exited zero but no validated artifact exists.

| Completion state | Evidence required | Meaning in the UI |
| --- | --- | --- |
| Source generation validated | Immutable originals, coverage, compiler and artifact checks | Downloadable reference; may still need a composite/target. |
| Target artifact validated | Above plus required parents, exact rendering and context checks | Generated for this consumer/target; eligible for activation only if resolver/target gates pass. |
| Model quality measured | Versioned task-suite results and intact drafts/responses | Separate first-pass and repair results; not universal expertise. |
| Effective / cache verified | Actual injection contract and qualified cache path | Serving observation tied to the current runtime, never inferred from compilation. |

“Perfectly designed” means all declared build contracts pass with no hidden loss or
unsupported claims. It cannot guarantee a small model answers every unseen problem.
Unused tokens are available capacity, not a requirement to add filler. Large,
high-quality reference content remains the objective.

### 1.2 Recipe and primitive boundaries

Use separate reusable Tool usages and pure-logic PythonCode components for each
operation. Each Rust-channel ToolSkill binding is immediately followed by its
associated orchestrator-channel PythonCode usage. Each component step references
one UUID. Skills describe one Tool usage and explicitly associate their executable
PythonCode; a Skill does not hide the whole generation workflow.

Reuse the current `host-assemble-prefix-bundle` Recipe, sweep/store primitives,
component stores, runtime admission/cancellation and existing shell primitive where
their contracts actually fit. Extend or version them rather than creating a second
prefix orchestration subsystem. Add a primitive only for an identified missing
operation, such as exporting a coherent catalogue snapshot or executing a bounded
compiler worker with artifact handles. Keep sequencing in Recipes.

The Python compiler makes no synthesis model calls. This does **not** make a Recipe
using `builtin.shell` Tier 0: shell Recipes are Tier 1 under the binding rules.
The current trusted internal-turn route is described as Tier-0-only. Verify and,
where necessary, implement support for an explicitly selected Tier-1 compiler
Recipe without permitting Tier-2 fallback. However, the first-build/rebuild path
must not depend on an LLM or an existing fresh prefix. Reuse an accepted deterministic
build/compiler adapter if its contracts fit; otherwise implement the missing bounded,
artifact-addressed compiler-worker primitive and its components before production
generation. This primitive runs the registered compiler with typed inputs and
returns artifacts; it does not sequence snapshot/validation/publication itself.
Accept the model-free Recipe path as Tier 0 only through its actual non-shell
execution contract. Do not relabel shell execution or bypass the orchestrator.

Record every proposed generation Recipe's exact LLM calls, prerequisites and tier.
The mandatory offline snapshot -> compile -> validate -> publish Recipe makes zero
LLM calls and admits work even with no generated prefix or configured provider,
using pinned local model/tokenizer artifacts. A missing artifact reports a specific
build prerequisite, not a provider request. Shell/Tier-1 variants remain separate
and cannot be the sole bootstrap path; any LLM steps require an already fresh usable
prefix and configured provider. No minimal/stale-prefix exception is introduced.

## 2. Precedence and reconciliation of the two older plans

Read this plan with [recipe.md](../../recipe.md), [skills.md](../../skills.md),
[tools.md](../../tools.md), [toolskills.md](../../toolskills.md),
[AGENTS.md](../../AGENTS.md), [simplified_v3.md](../../simplified_v3.md) and the
[development policy](../development-policy.md). Those binding contracts override
obsolete examples in the plans below.

| Earlier source | Preserve | Correct or replace |
| --- | --- | --- |
| [Archived Prefix Cache V3](../archive/prefix_V3.md) | Persist assembled content, stable fingerprints, cheap serving reads, separate Kohai/Sempai consumers and an operator Prefix tab. | Its completion marks are historical claims. Named entries are not supported by the existing scope-only unique key. Generation must not depend on an optional Sempai connection. |
| [Prefix Plan 2](../../prefix_plan2.md) | Recipe-owned sweep/store flow, validation gating independent of consumer tags, real class codes, fail-closed internal jobs, request-specific completion checks and concurrency fencing. | Its multi-UUID steps, source interpolation and mandatory fresh-step state are superseded. Do not adopt its old `host.*` capability IDs, arbitrary row caps, or architecture-heading slices. |
| Current domain compiler work | Literal evidence cards, raw snapshots, checksums, mandatory coverage, immutable bundles, actual tokenizer/template fingerprints and hybrid-cache measurements. | Domain compiler assertions and adapters must be separated from the reusable engine. Citation/configuration helpers are client checks, not automatic enforcement in every BrassClaw model call. |

The Python namespace remains `host.sweep_validated_components` /
`host.store_prefix_bundle`; their current capability IDs use `builtin.*`.
V088 repaired that distinction. Scope tickets and generation leases currently
fence a specific internal operation. They must not become extra Tool permission
grants. The v3 operator token and live instance-wide Tool policy control authority;
legacy tenant/user/agent/project columns are compatibility data until their
coordinated migration, not justification for new feature-role checks.

## 3. Current code: verified seams and shortcomings

This is a source inventory, not a claim that every path has passed production
acceptance during this planning task.

| Area | Current implementation | Upgrade consequence |
| --- | --- | --- |
| UI | `prefix-tab.js` and `usePrefixes.js` already render entries and distinguish Generate from Regenerate. | Extend this surface; do not add a parallel settings page. |
| Browser API | `settings-api.js` calls the v2 prefix routes. | Retain the same facade-backed API family. |
| HTTP | `handlers.rs`, `descriptors.rs`, `router.rs`: `GET /api/webchat/v2/prefixes`, `POST /api/webchat/v2/prefixes/{name}/regenerate`. | Preserve ingress policies and route contracts; add durable job/detail/activation operations. |
| Facade | `RebornServicesApi` delegates to `InterceptorConfigService` in `reborn_services/interceptor_config.rs`. | Extend the existing service seam and DTOs; no direct DB/compiler access from HTTP handlers. |
| Service | `RebornInterceptorConfigService::list_prefix_entries` returns one base-prompt entry; `regenerate_prefix` rejects other names. | Register all six seeded profiles and supported custom profiles and dispatch by validated profile identity. |
| Generation | `regenerate_prefix` creates an internal conversation, scope ticket, lease and request UUID, submits `host-assemble-prefix-bundle`, checks terminal reply and exact stored `generation_id`. | Preserve durable operation fencing and failure semantics; replace the long synchronous HTTP wait with job admission/status. |
| Sweep | `PgSweepValidatedComponentsBackend` selects validated rows from fixed tables without the old consumer-tag exclusion; sorts by class/prompt UID. | Reuse the read boundary, add one consistent snapshot and complete semantic records. |
| Lost information | Sweep expressions prefer `prior_knowledge_content`; Recipes contribute only prior knowledge or an empty string. Tool schemas, Recipe variants/bindings and associations are absent from the rendered body. | Export complete original content and structured execution metadata; summaries cannot replace them. |
| Truncation | Sweep SQL has `LIMIT 1000` per table and runs separate queries without an enclosing consistent-snapshot transaction. | Export the complete eligible catalogue with stable pagination under one snapshot; do not silently mix generations or omit rows. |
| Docs | Sweep appends compiled-in CLAUDE/AGENTS slices starting at architecture headings. | Include complete current authoring guides and required precedence sections, including material before those headings. |
| Role contamination | Sweep appends the Sempai response schema to the common bundle. | Keep role-specific response instructions outside the shared factual corpus. |
| Store | `PgBasicPromptStore` persists `bundle_json`, fingerprint, timestamps and a request `generation_id` under one scope key. | It has neither a prefix-name key nor immutable generation history/evidence metadata. Request UUID is not the compiler's content-addressed generation ID. |
| Serving | `PgKohaiPort::complete_with_stores` resolves a stored bundle; `run_sempai_review` has `SystemBundleSource`. | Trace and test both independently. The archived claim that Kohai runs on every turn must not cause model calls on deterministic Tier-0 tasks. |
| Staleness | Q2 approval calls `mark_stale`; current `get_system_bundle` uses a minimal fallback for stale rows or DB errors and has a default-project probe on a miss. | Add dependency-aware invalidation and a deliberate serving policy; do not silently discard a large usable generation during a rebuild. |
| Startup | `component_boot::initialize_runtime_components` seeds host and first-party components and checks integrity; runtime initialization calls it. | Seed reference sources through this shared boot path, not only on WebUI mount. Global Monty lifecycle acceptance remains the separate Phase-3a prerequisite. |
| Citation helpers | All three domain compiler `--mode run` entry points use `run_compiler_task`; the standalone shared CLI verifies the server token prefix. | Carry evidence IDs/manifests into production provider/answer paths. Existing direct vLLM calls remain unchecked. |
| Standalone compiler execution | Sempai has legacy triage/distill/run modes as well as offline context compilation; the offline switch applies only to context mode. Runtime inspection uses local process flags. | A production worker must expose a pinned model-free build contract, not execute `--mode all`, inherit arbitrary environment settings or mistake its own host for the inference host. |
| Hybrid cache evidence | LMCache 0.5.5 / vLLM 0.31.0 on the inspected Ornith server produced corruption on external restores; separation alone failed, then an upstream adapter backport passed sentinel restores. | Make runtime/cache-layout correctness an explicit target prerequisite; package versions and native hit counts alone are insufficient. |

Relevant inspected files:

- [`interceptor_config_service.rs`](../../crates/brassclaw_reborn_composition/src/interceptor_config_service.rs), [`pg_prefix_bundle_backends.rs`](../../crates/brassclaw_reborn_composition/src/pg_prefix_bundle_backends.rs), [`pg_basic_prompt_store.rs`](../../crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs), [`pg_prefix_scope_ticket.rs`](../../crates/brassclaw_reborn_composition/src/pg_prefix_scope_ticket.rs).
- [`builtin_bootstrap.rs`](../../crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs), [`component_boot.rs`](../../crates/brassclaw_reborn_composition/src/component_boot.rs), [`pg_docus_store.rs`](../../crates/brassclaw_reborn_composition/src/pg_docus_store.rs), [`factory.rs`](../../crates/brassclaw_reborn_composition/src/factory.rs), [`runtime.rs`](../../crates/brassclaw_reborn_composition/src/runtime.rs), [`webui.rs`](../../crates/brassclaw_reborn_composition/src/webui.rs).
- [`pg_kohai_port.rs`](../../crates/brassclaw_reborn_composition/src/pg_kohai_port.rs), [`loop_driver_host.rs`](../../crates/brassclaw_reborn/src/loop_driver_host.rs), [`serve.rs`](../../crates/brassclaw_reborn_cli/src/commands/serve.rs).
- [`interceptor_config.rs`](../../crates/brassclaw_product_workflow/src/reborn_services/interceptor_config.rs), [`prefix-tab.js`](../../crates/brassclaw_webui_v2_static/static/js/pages/settings/components/prefix-tab.js), [`usePrefixes.js`](../../crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/usePrefixes.js).

## 4. Six seeded profiles, custom profiles and one reusable compiler engine

| Profile ID | UI label | Compiler entry point | Source content |
| --- | --- | --- | --- |
| `base-prompt` | BrassClaw Base Prompt | `base-promt-compiler.py` (offline development adapter) | Current release docs and complete approved component snapshot. |
| `defensive` | Defensive / Servers and Networks | `Defensive-compiler.py` | Existing defensive source families and security/maintenance evidence. |
| `homeassistant` | Home Assistant / MQTT / Modbus / YAML | `Homeassistant-compiler.py` | Existing official domain sources and coverage gates. |
| `tomedo` | tomedo / API / macOS / PostgreSQL / Billing and Coding | `tomedo-compiler.py` | Existing official docs, qualified forum observations and curated local findings; no clinical knowledge. |
| `sempai` | Sempai / Prompt Review and Component Authoring | `sempai-compiler.py` | Complete BrassClaw authoring/validation references and explicit prompt-diagnosis, Recipe and component-proposal procedures. |
| `prefix-maker` | Prefix-Maker / Prefix Design | `prefix-maker-compiler.py` (offline adapter; runtime qualification pending) | Versioned prefix engineering, evidence/formatting, model/tokenizer/template and cache architecture references, measured design lessons and the supported profile-design contract. |

Keep `base-promt-compiler.py` as the requested filename; the canonical profile name
is correctly spelled `base-prompt`. Paths are registry-owned, not browser-supplied.

The offline base adapter now reuses extracted HA evidence/rendering functions in
`scripts/prefix/prefix_evidence.py`, rather than retaining another monolithic copy.
Any remaining profile migration must replace **all** HA-specific source lists,
domain/platform tags, coverage gates, routing patterns, headers, runtime guidance,
default cache paths and usage instructions. Add release-document and component-
snapshot input adapters. Do not leave a Home Assistant topic gate or system prompt
in the new base compiler. Do not rename or replace the existing domain compilers.

After obtaining behavior parity, extract the common evidence/assembly machinery
into a shared module. Keep small profile entry points and profile-specific source
adapters. Preserve independent caches, raw sources and historical generations.
Sharing a rendering engine must not mix domain source databases or approval rules.

Current parity is limited: the Sempai compiler has the v25 scoped artifact audit,
reviewed strict-JSON display projection and evidence-view manifest; the independent
Home Assistant, Defensive and tomedo compilers do not yet share those features.
The base entry point now exists as an offline development adapter; Prefix-Maker
still has no compiler entry point. Port and qualify
these evidence/rendering facilities in the shared engine before claiming parity.
Keep Sempai's reviewer/planner schemas in its consumer adapter, not in domain answer
contracts. Inventory each profile's collection modes, source configuration, coverage
and validation adapters; generic domain-source environment variables are not a
supported way to configure the current Sempai compiler.

Builds expose explicit input/output directories, pinned model/tokenizer artifacts
and a machine-readable result. An offline build from a fixed source snapshot is
deterministic and uses no model synthesis. Source refresh is a separate observable
stage, with its own network permissions, recorded revisions and failure outcomes.

### 4.1 Production compiler contract and artifact layout

Extract a library/worker interface consuming only immutable registered inputs:
profile/policy revision, coherent source export, exact component dependencies,
verified teaching package where applicable, model configuration, tokenizer files,
chat-template adapter and target capability revision. Paths are internal artifact
handles. Build-time refresh, model calls and live host/process inspection are
excluded; the scripts' synthesis/doctor/run/reset modes are not production build
entry points. Test the real build with provider/network access unavailable, not
merely a declared offline flag. Imports must not start clients or fetch sources.

Specify supported Python/library versions, installed dependencies, CPU/RAM/disk/
time limits, bounded child-process/output handling, cancellation, stage checkpoints
and atomic staging/publication. Compilation is primarily parsing, evidence
selection and tokenization; an NVIDIA GPU is not a prerequisite. Worker placement
is an execution choice, not a browser host/path field or new provider catalogue.
Resume verified stages; never publish a killed compiler's incomplete export.

| Logical artifact | Required contents |
| --- | --- |
| Input manifest | Canonical release/catalogue/source/policy/compiler/tokenizer/template identities and all pinned dependencies. |
| Raw source package | Complete original bytes, source disposition, selected/omitted whole-unit map and licensing metadata. |
| Evidence index | Stable IDs, original ranges/fields/revisions, original, display and rendered hashes, the pinned transformation/mapping manifest, applicability and navigation. |
| Model input | Stable reference text and registered client/server rendering adapter; no job IDs, live conversation or fetch timestamps. |
| Token manifest | Actual reference/shared/full-envelope token counts and leading-token hashes across supported variants; runtime cache assumptions separate. |
| Build validation receipt | Coverage, artifact integrity, source/teacher checks, capacity, deterministic reproduction and specific blocked/error reasons. |
| Quality/cache observations | Append-only receipts bound to generation and actual validator/provider/runtime revisions; never substituted for originals. |

Use relative artifact names or content-addressed references. Host checkout/output
paths, download locations and timestamps belong to operational records, not the
content ID or stable model text. Avoid hash cycles: establish generation identity
from canonical immutable inputs/rendered payloads, then attach receipts to it.
Different output directories/worker hosts must not change evidence IDs, compiled
text, templates or leading token IDs.

### 4.2 Current Sempai build configuration and creation instructions

This is the audited standalone v25 procedure, not an implemented WebUI job or a
portable interface for every model. Production Generate must resolve equivalent
immutable typed inputs from the accepted design and source export; it must not
inherit shell environment or discover processes. Record the compiler file digest
and pipeline revision `2026-10-09-sempai-artifact-scopes-v25` with every build.

| Current setting | Actual behavior and production requirement |
| --- | --- |
| Context target/reserve | Defaults are 100,000 requested prefix tokens, 131,072 context tokens and 31,072 reserved tokens. Reserve must be at least 8,192. Effective capacity is the lesser of the request and model context minus reserve, checked against rendered envelope variants. The experimental 114,688 / 16,384 split is an explicit Ornith design choice, not the default or a guarantee of fit. |
| Model/tokenizer | `ORNITH_MODEL_CONFIG` identifies local config; the tokenizer loads offline from that resolved file's parent directory. Supply a coherent pinned config/tokenizer directory. `TOKENIZER_MODEL` does not replace this selector. The current adapter requires the inspected 24-linear/8-full architecture and single-GPU FP8 attention geometry; a new model requires a supported adapter, not just a new DB design. |
| Source inputs | `SEMPAI_SOURCE_ROOT` supplies the declared complete source package and teaching dependencies. Optional `SEMPAI_SOURCE_MANIFEST` pins the captured manifest. `CODING_REPOS` and `CODING_WEB_SOURCES` are rejected. Collection retains all declared originals; selection changes only the model-facing view. |
| Reviewed selection | `SEMPAI_REFERENCE_POLICY` must match the current document/excerpt hashes and whole-unit coordinates, inventory every unit and satisfy mandatory/topic coverage. Without it, the compiler renders literal originals; strict-JSON display projection is enabled only with the reviewed policy. Stale v22/v24 policies cannot qualify edited v25 sources. |
| Offline mode | Collect, distill and reduce are deterministic source stages with no synthesis calls; context uses `--offline`. Offline geometry is an assumption recorded as unverified serving configuration, not a cache qualification receipt. |
| Online discovery | The current context preflight combines target `/models` with process inspection on the compiler host and requires explicit hybrid flags and exact chunk/block equality. This is stricter than some supported runtime configurations and unsuitable for proving a remote target. Replace it with a versioned target-evidence adapter; do not modify operator services to satisfy the heuristic. |
| Identity/publication | Current generation identity includes compiler bytes and profile fields containing local artifact paths; source records also carry checkout provenance. Cross-host/path independence in §4.1 is a migration requirement, not demonstrated current behavior. Generation publication and the local `current` pointer do not activate a provider prefix. |

The current reviewed-policy gate requires complete model-visible `recipe.md`,
`skills.md`, `tools.md`, `toolskills.md`, `sempai-authoring-reference.md`,
`sempai-monty-compatibility.md`, `sempai-worked-examples.md` and
`crates/brassclaw_interceptor/src/packet.rs`; source/topic coverage gates additionally
apply to the selected corpus. Other whole units may be omitted only under reviewed
selection while their full originals remain retained. The schema-1 policy status is
`reviewed-source-selection`; each unit records path, line bounds, document/excerpt
hashes and an exact boolean selection. Its status label alone is not review evidence.
The present renderer orders ordinary sources before verified worked examples and
the compatibility overlay, followed by dispatch navigation, the source-heading index,
factual checks, scoped artifact audit and request boundary. This ordering is currently
compiler code, not an arbitrary provider-generated layout option. Port supported
layout changes explicitly and qualify them before exposing them as accepted designs.

Before running the example, supply an existing dedicated compiler Python environment
with the declared dependencies, a coherent source package including official Monty
artifacts and verified teaching receipts, local model/tokenizer files and a newly
reviewed selection policy for that exact package. The placeholder paths must be
replaced; use a fresh build directory. Do not install into or change the operator's
vLLM environment. Reuse unchanged teaching evidence only when its complete pinned
checker/dependency combination still matches; revalidate affected examples after
source drift. Never repair a stale policy by automatically rewriting its hashes.

```sh
export PREFIX_CHECKOUT=/path/to/brassclaw
export PREFIX_PYTHON=/path/to/compiler-environment/bin/python
export SEMPAI_SOURCE_ROOT=/path/to/coherent-source-package
export ORNITH_MODEL_CONFIG=/path/to/pinned-model-and-tokenizer/config.json
export CODING_DISTILL_CACHE=/path/to/new-sempai-build-directory
export SEMPAI_REFERENCE_POLICY=/path/to/reviewed-current-source-policy.json
export MODEL_CONTEXT_LIMIT=131072
export PREFIX_WORKSPACE_RESERVE_TOKENS=16384
unset CODING_REPOS CODING_WEB_SOURCES
# Remove an inherited source-manifest setting unless it pins this exact package.
unset SEMPAI_SOURCE_MANIFEST

"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode plan
"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode collect
"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode distill
"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode reduce
"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode context --offline --context-tokens 114688
```

For a source-manifest-pinned build, explicitly set `SEMPAI_SOURCE_MANIFEST` to the
matching immutable manifest instead of unsetting it. Do not add `--offline` to the
other stages: that flag is accepted only by context. Avoid `--mode all` for this
procedure: its context call currently uses defaults rather than the CLI context
request. Legacy triage/run modes are not required to create this exact-source prefix.
If mandatory selected units exceed capacity, retain the failed build and resolve
coverage/model capacity; the example does not authorize automatic truncation.

Publish and inspect the immutable `generations/<generation-id>/` artifacts, including
`context_100k.md` (a legacy filename, not its measured size), template, source/evidence
manifests, policy, teaching receipts and token/build manifest. Pin those paths rather
than the mutable top-level aliases or `current` pointer. Complete original documents
remain under the build's `raw/documents/` and teaching material under `raw/teaching/`;
the generation directory alone is not the complete raw source package. Production
publication must retain that package separately with integrity and retention links.
A build's `external_cache_reuse_verified=false` cannot become a warmed/qualified badge.

Consumer schema/grammar generation is a separate protocol artifact step. Pin exact
ordered schemas, grammar compiler/runtime identity and decoder options; regenerate
incompatible planner/reviewer grammars. The retained v25 protocol checks do not
constitute a newly generated prefix or model inference result. Optional provider
planning and quality evaluations remain explicit Tier-1 workflows, never hidden
inside model-free Generate.

## 5. Base-prompt knowledge: complete originals and applicability

### Mandatory current documentation

Include complete `skills.md`, `recipe.md`, `tools.md`, `toolskills.md`, `CLAUDE.md`,
`AGENTS.md` and `simplified_v3.md`. The actual filenames are case-sensitive on Linux;
map user-facing lowercase names to their real paths. Do not rely on macOS case
folding or architecture-heading slicing.

Create a versioned inventory of all other documentation for the current BrassClaw
release: current `docs/agents-v3`, subsystem contracts and crate `AGENTS.md` /
`CLAUDE.md`, current supported installation/configuration/API/operations references,
and relevant runtime schema/capability definitions. Inventory every candidate and
record whether it is included, superseded, historical, development-only or private.
Curated inclusion rules must have reviewable reasons, not silent omissions.

Archive material and implementation plans are not current runtime truth. Preserve
their provenance where useful, but label historical examples and unimplemented
targets. `prefix_plan2.md`, `docs/archive/prefix_V3.md` and this upgrade plan must not
become evidence that a target feature is already implemented. Keep development-only
rules separate from runtime operator guidance. Exclude `LOCAL_TEST_ENV.md`, secrets,
attachments, private deployment details, build products and patient data from the
shipped reference package.

### Components themselves

Export all eligible activated/approved components and the structured data needed
to understand them, not just `prior_knowledge_content`:

- Tools: stable UUID, real class, callable/capability identity, complete description,
  parameter/result schemas, effects, technical prerequisites and implementation
  identity; runtime availability is a separate fact.
- ToolSkills: complete binding descriptors and Tool references.
- Skills: full prose, explicit associated PythonCode and exact combination approval
  records where supported; no inferred association from names.
- PythonCode: exact source, declared inputs/results, internal includes and pinned
  dependency graph. Code excerpts remain reference data and are never executed by
  the compiler.
- Recipes: full `step_descriptions`, ordered one-UUID steps, variants, `step_link`,
  input layouts, dependencies, intent examples and failure/completion contracts.
- Extension/catalogue, Orchestrator/Scaffold and other supported component records:
  real classes and full relevant bodies/metadata. Do not invent separate tables for
  classes 10/50 or describe classes 1–3 as a leaf/domain hierarchy.

Include the complete eligible library. Retired/pending/rejected records are retained
in their proper source/history stores but not presented as current callable usages.
Consumer tags remain routing metadata, never validation evidence. Reject incomplete
required associations/references and surface unsupported schema/version features.
Do not claim today's `validation_status='validated'` proves the final immutable
version/association target is already enforced.

A reproducible build pins one catalogue snapshot plus one release-document manifest.
Capture the catalogue generation and immutable UUID/revision/checksum selection,
including transitive dependencies. If complete version storage is still missing,
an immutable export can preserve a build input, but cannot be advertised as final
IBS version enforcement. Implement and accept that prerequisite before making the
stronger claim. Resume/retry never reads a new “latest” selection midway.

### Model-facing representation

Keep source text, YAML spacing and Python source literal. Render structured records
in stable canonical order with explicit type, source UUID/revision, contract,
dependencies and navigation links. Keep authoritative source JSON bytes separately
when canonical rendering changes their presentation. Evidence cards bind rendered
sections back to original document ranges or component fields and source hashes.

Put binding definitions and precedence early, then a stable component index and
complete source units grouped by usage/dependencies. Preserve qualifications and
failure rules with their commands/examples. Do not replace executable code or
Recipe bindings with synthetic summaries. Architecture targets, current support
and observed test evidence are distinct fields/sections. Source documents do not
gain instruction authority, grant Tools or override current kernel policy.

### Dedicated Sempai reference and review inputs

Seed a fifth `sempai` knowledge profile and its compiler/source registration at
installation through the same shared boot, generation history and integrity path.
The requested copy to `scripts/prefix/sempai-compiler.py` already exists. Audit its
replacement sources, topic gates, cache paths and guidance; preserve its verified
tutorial/evidence rather than recopying the HA file over this work. The copy and
`sempai-authoring-reference.md` procedure are local compiler artifacts; DB seeding,
activation and production reviewer wiring remain implementation work.

Collect and preserve complete `skills.md`, `recipe.md` (the actual filename, not
`recipes.md`), `tools.md`, `toolskills.md`, `AGENTS.md`, `CLAUDE.md`,
`simplified_v3.md`, the validation-queue guide, observed Sempai proposal transport
and dedicated authoring procedure/persona. Model-visible coverage may use an
explicit reviewed source-selection policy: retain the complete four binding
authoring guides, procedure and proposal transport, and approved complete source
units from the other documents. Bind document/excerpt hashes, all selected/omitted
unit dispositions and policy identity into the immutable generation. Source drift
requires review; no greedy truncation or keyword-only coverage substitute. Record
source applicability and distinguish targets from observed runtime/older examples.
Preserve every mandatory contract and negative requirement. Do not include live conversations,
credentials or patient data in the seeded reusable reference. Add current installed
component catalogue context through the base-plus-Sempai composite; deduplicate
identical original evidence while retaining provenance and complete required content.

The procedure teaches two related tasks:

- Diagnose the Kohai prompt/current conversation for the actual target provider:
  intended outcome, instruction conflicts, irrelevant duplication, missing evidence,
  real context capacity, thinking/tool/structured-output capabilities, role ordering
  and exact tool-call/result relationships. Preserve user intent, observations,
  incomplete effects and uncertainty. Adjust only the permitted volatile prompt;
  never rewrite the task's pinned base or infer unknown provider capabilities.
- Propose reusable components, especially Recipes: discover existing usages before
  authoring, define variants and typed input/result flow, one UUID per component
  step, immediate ToolSkill/PythonCode pairing, internal dependencies and supported
  constructor fields, ten-plus intent examples, exact Skill associations/approval,
  failure/retry/effect contracts, semantic/behavioral review, Q1 and human Q2,
  coherent activation, immutable retention and dependency-aware prefix rebuilding.

Extend the trusted review packet through the existing interceptor/gateway seams to
carry the Kohai request, current conversation, packet/run/turn identity, pinned
workflow/prefix references and DB-resolved target provider/model revision and
capabilities. Distinguish this target from the provider serving Sempai itself.
Preserve structured roles, tool envelopes and call/result IDs instead of reducing
them to text pairs. Redact secrets through host handling. The current role/content
pair input does not supply all these facts; missing metadata is an explicit gap,
not permission for Sempai to invent it. Distinguish pre-call prompt review from
post-response learning: pre-call review cannot observe a response not yet produced.

Sempai produces drafts through the current proposal sink; it cannot approve itself,
mutate live component versions, grant Tools or activate settings. Use actual
class-specific proposal/store schemas and trusted draft identities; missing APIs
block that proposal. Preserve `SempaiReviewOutcome`/persona response contracts as
role-specific instructions outside the factual prefix. Update their versioned
schema/persona together where needed; the older persona example must not override
the current `proposed_components` transport or introduce unsupported insert fields.
The standalone compiler's citation `--mode run` is a factual benchmark only and
cannot replace the production reviewer/proposal response schema.

Bind the base-plus-Sempai composite to the Sempai consumer independently of Kohai's
selected domain. Keep both provider selections DB-only and pin review generation
and target request metadata through continuations. Add no Sempai model invocation
solely because a deterministic Tier-0 task completed. Reference readiness, review
enablement, proposal validation and provider/cache observations are separate states.

Initial standalone evidence (2026-10-06): eleven full source documents, 247 validated
source units and 542602 source bytes. The pinned local Ornith tokenizer's sum of
mandatory rendered cards is approximately 184990 tokens before the final envelope,
already above its 131072-token context and the explicit 100000-token preview target.
The compiler correctly reports Capacity blocked; no generation was published or
deployed. See [the capacity receipt](../../scripts/prefix/sempai-capacity-verification.json).
This is not a shared-token/cache measurement or production acceptance. A larger
compatible target or an explicitly reviewed source/partition policy is required;
complete mandatory guides cannot be silently omitted to fit this deployment.

Reviewed standalone deployment (2026-10-06): the explicit selection policy now
compiles 119/247 preserved units into a 67432-token reference (67700 maximum
rendered probe tokens). It was deployed to the existing `brassclaw` vLLM template;
exact server token IDs and 66528 native cache-hit tokens were verified. Fourteen
direct-model reviewer probes from `brassclaw2` demonstrated a port-validator draft
passing 13/13 independent Linux CPython cases after feedback and an offline v3
Recipe design passing structure/intent checks. Initial JSON/proposal failures,
duplicate/incomplete drafts and a malformed blocked Recipe remained important
negative evidence. This is supervised draft generation, not production Q1/Q2,
Monty compatibility, DB seeding, automatic proposal insertion or consumer wiring.
The audited sink supported classes 21/22 but discarded v3 Recipe fields. The
subsequent [draft-persistence repair](../reborn/contracts/sempai-draft-persistence.md)
retains supported constructor fields, rejects unknown fields and commits the
legacy draft/queue together. This does not establish immutable review admission,
complete class-specific validation, association support or production approval;
those prerequisites remain required before activating v3 proposals. See
[the live test report](../../scripts/prefix/sempai-evaluation-20261006/README.md).

### Evidence learned since that initial deployment

The v14 standalone reference has 97,822 text tokens, 97,828 shared leading tokens,
92 complete 1,056-token blocks (97,152 cacheable tokens) and a 676-token shared
tail. Its 29 complete teaching examples pass 132 Linux behavioral probes with
deliberate mutations rejected. These are teaching artifacts, not activated
components or Monty/production Q1 proof. See the
[retained experiments](../../scripts/prefix/sempai-accuracy-20261007/README.md).

Unchanged-client v13 measured 54/60. V14's different two-call planning, strict-schema
and input-presentation protocol measured 55/60 first pass and 60/60 after up to two
feedback repairs. Additional development transfer cases measured 10/12 first pass
and 11/12 after two repairs. Neither protocol changes nor repair acceptance may be
advertised as prefix-only gains or flawless first-pass ability. Healthy native-only
semantic failures remain distinct from cache serving failures. V14 is standalone
Sempai: its capacity does not prove a full installed base-plus-Sempai composite fits.

### 2026-10-09 Monty/compiler evidence and promotion limits

The complete pinned official Monty source package and compatibility overlay now
compile with all four current authoring guides. After concurrent source changes,
the latest isolated v22 generation has 113,903 text tokens, 113,909 shared leading
tokens and 107 complete 1,056-token blocks (112,992 cacheable tokens); its maximum
build-time rendered probe is 114,171 tokens, with 16,384 tokens reserved. These
measurements belong to Ornith's inspected tokenizer/template and standalone Sempai,
not a full installed base-plus-Sempai composite or a universal model design.
The current v22 priming check verified exact automatic server injection and
112,992 native cache-hit tokens. Plain external restoration was exercised after
GPU displacement: zero native hits, 112,992 extra LMCache hits and the correct
sentinel answer. This evidence covers the inspected LMCache MP in-memory path,
not persistence across service restarts or unrelated cache layouts. The structured
external-restoration check also passed with the same cache counts and correct READY
JSON. Keep quality checks and production qualification separate.

The preceding fixed v19 design/content trial scored 58/60 first pass versus 55/60
with the prior two-call protocol, with five improvements and two Recipe regressions.
All 24 pure-logic candidates passed 318/318 probes in actual upstream Monty 1.0.0.
Transfer scored 10/12 first pass and 11/12 after two bounded repairs. Monty reference
answers had 22/22 correct booleans but only 17/22 source-topic-supported citations
after manual audit, including one corrected evaluator spelling false negative.
Retain the original receipts; valid citation IDs alone did not prevent selection
of an unrelated passage. See [the retained experiment report](../../scripts/prefix/sempai-accuracy-20261009/README.md).

These observations establish source/format and limited development behavior,
not flawless performance, a no-regression promotion or production Q1/Q2. Current
source refreshes need their own qualification; do not relabel v19 scores as v22
scores. Prefer complete general teaching and actual artifact-level checks over
adding benchmark-specific answers. Require helper returns, missing-only defaults,
and review/sink prerequisites in the emitted artifact itself. Repair evidence stays
separate and cannot conceal first-pass regressions. Actual custom Monty loader,
Tool binding, full approved component associations, proposal persistence and the
installed consumer path remain independent production acceptance gates.

The refreshed v22 knowledge trial completed 27/28 JSON answers, with all 27 boolean
answers correct; one response was truncated and stays failed. Its original citation
topic check passed 18/28. Manual review identified three narrow wording/newline
matcher false negatives, yielding 21/28 supported answers, with six unrelated
passage selections still rejected. Preserve both original and audited receipts.
Recompilation of the identical pinned v22 inputs reproduced its exact generation
and manifest. This does not convert the old 58/60 trial into a current-content
score or remove the no-regression/production prerequisites.

The same-source v23 display/navigation experiment preserves v22's exact original
evidence and selected 220 units. Lossless strict-JSON display compaction saves
5,633 tokens; a source-heading index and general artifact audit use that space.
All 28 reference responses are valid JSON, 27 booleans are correct and the
unchanged topic-support checker accepts 23 complete answers; correct explanations
with reversed booleans and unrelated citation IDs remain failures. Frozen first
pass is 57/60, transfer 11/12. Both earlier Recipe-prerequisite failures are fixed,
but two validators never enter a success state and one Recipe repeats an intent.
This is development evidence, not accepted no-regression promotion. Retain the
failed drafts and comparison instead of claiming a perfect design. The following
v24 candidate groups navigation by document and strengthens contract-versus-plan,
valid-path, cardinality and boolean-proposition audits. Its fixed first-pass suite
passes 59/60, transfer 10/12 and strict reference checks 25/28. The missing-only
default remains incorrect; one transfer request emits no requested Recipe, and
the MCP server-lifetime answer still contradicts its own explanation. Preserve
these failures. Actual upstream Monty checks pass 302/302 probes for the 23 frozen
programs admitted by the pure-Python gate, not all 24 requested programs; transfer
passes 33/34. This is not flawless or complete production qualification.

Qualification must also check decoder field order against the accepted design.
The historical v24 evaluation schema emitted `composition_summary` before
`proposed_components`, contrary to the prefix's instruction to summarize after
checking the final artifact. Reference output emits the boolean before its reason.
The v25 evaluator now emits proposed_components before composition_summary, with
the summary last; other field constraints remain unchanged. Grammar manifest v2
fingerprints the ordered schema and enforces fixed property order. The loader
rejects old reviewer grammars with stale ordering, while retaining compatible v1
artifacts only when their exact schema/layout still matches. New planning
schemas necessarily require regenerated grammars. Local tests and installed
XGrammar 0.2.7 matching verify accepted artifact-first and rejected summary-first
output. This fixes a protocol contradiction, not a demonstrated accuracy score.
Evidence/reason-before-verdict for reference questions remains a separate future
change. Preserve baseline artifacts and distinguish client-format changes from
prefix-only improvements.

The v25 authoring procedure and compiler classify host embedding examples,
preload definitions, invocation bodies, Recipe constructors and prompt review
separately. Module-level result/invocation requirements apply to invocation bodies;
preload definitions expose typed returns without running at load. Retain upstream
Monty originals and verified teaching programs. Optional planning now returns
exact current-contract quotes with aspect labels and supplied Recipe references,
not invented executable branches. The host verifies quote provenance and reference
identity; artifact classification, coverage and semantic agreement still require
review of the complete original contract. Neither valid quote selection nor JSON
shape proves code behavior or authorizes activation. Empty compatibility arrays
never cancel requested component drafts; offline constructor support is distinct
from live execution/sink limitations. Fresh source capture and normal policy/integrity
qualification precede compilation; these edits do not rewrite published generations
or change the deployed prefix automatically.

The v24 paired MCP-lifetime probe verifies a cold miss, 112,992 native reused
tokens and an external restore with zero native hits and 112,992 extra LMCache
tokens. The same incorrect boolean and correct explanation appear in all paths;
this particular polarity failure is reproducible without cached execution.
The retained `v24-lifetime-semantic-cache.json` and its separate audit establish
only this fixed question, not general cache correctness. Keep task/grammar/content
identical across cache paths and score semantics independently. A short READY
sentinel alone cannot qualify Sempai behavior.

The paired v24 missing-default writer probe also verifies cold/native/external
paths, with 114,048 tokens externally restored and zero native hits. All four
writer stages emit exactly the same code, bound by SHA-256 to the retained Linux
and upstream Monty checks (6/7 behavior probes pass; absence fails). Reuse that
exact-code evidence explicitly instead of implying new executions. This holds
the already-generated untrusted plan fixed; it does not test planner regeneration
or prove correctness of all restored tasks. Both paired cases reproduce their
failures cold, so these particular errors cannot be attributed solely to cache
restoration. The active startup log also confirms automatic `align` mode.

### 5.1 Knowledge quality and model-facing structure

Include a pinned official Monty documentation package and a separate BrassClaw
compatibility overlay. The current standalone package is upstream v1.0.0 at
`85c5d1f6bef038405cfc40a4eed94806e303567e`, with 61 original artifacts and retained
MIT licensing. Preserve all originals; rendered API-template directives alone do
not establish callable signatures. Bind upstream version, selected custom Monty
revision, host ABI and effective constraints independently. Cover language/module
limitations, eager iterable behavior, host-value conversions, private transport
limits, cumulative task accounting, trusted snapshots and durable effect recovery.
Upstream support for eval/exec or filesystem callbacks is not BrassClaw permission.

Distinguish effect-free Skill definition preloading from pinned export invocation
and standalone step-body artifacts. Validators must select the actual artifact
kind: preload definitions must not dispatch Tools, while an invocation captures
the function's complete returned result. Do not require module-level invocation
inside a preload-only artifact or accept an uncalled helper as a completed step.
Pin export names/signatures, dependencies and isolated mutable state through the
real loader/runner contract. MCP sends a completed listed command through an
ordinary chat; it is not a direct Monty/IBS/Tool execution endpoint. Historical
tutorials retain their dated applicability and do not override current guides.

Version each profile's source and teaching policy. Require a coverage matrix mapping
each promised capability to authoritative source units, applicability and checks;
heading/keyword matches alone are insufficient. Sources record release, product/
protocol version, licensing/distribution disposition, original hash and review
status. Official specifications and project contracts take precedence; forums,
cookbooks and local findings are qualified observations with prerequisites and
limitations. Resolve conflicts explicitly rather than giving every source equal
instruction authority. Refresh failures cannot silently substitute unreviewed
content or manufacture current-version claims.

For base/Sempai, include complete required authoring/schema/association/retry guides
and approved catalogue records. Defensive coverage includes evidence preservation,
incident response/backdoor/rootkit investigation and reversible maintenance with
actual platform/version context. HA covers YAML indentation/typing, automation/
script semantics, MQTT session/QoS/retention and Modbus addressing/data interpretation.
Tomedo preserves versioned API, macOS/Postgres and dated German billing/coding
sources, distinguishes local discoveries and excludes clinical knowledge. None of
these references grants operational authority.

Use one predictable envelope: trusted role/authority and precedence, navigation,
applicability/contracts, literal evidence units, verified general teaching units,
then one short task-independent dispatch/checklist boundary before volatile input.
Keep the shared base spine stable; consumer instructions belong to the registered
trusted envelope, not duplicated personas inside factual source units.
Test stability across knowledge updates as well as request variants. The current
renderer places the complete source inventory and version-dependent document-card
IDs before their evidence text; a new component or a late document edit can
therefore invalidate an otherwise unchanged leading core. A successor accepted
model design should place stable core material before changing catalogue/control
metadata and move versioned citation/provenance indexing after the relevant stable
text where the supported renderer permits it. Preserve version-bound citations
and exact origin reconstruction; do not substitute path-only identities to inflate
hit counts. Trial an appended component and a late-source edit with unchanged
mandatory knowledge, record full framed token-prefix changes, and qualify hybrid
reuse separately. Implement a new versioned renderer/design contract; do not alter
frozen literal/v1 or v2 semantics silently. Repeated transfer envelopes may use a
lossless reference index only when every original value/definition is retained,
references resolve exactly and the complete raw envelope remains archived. The explicit v3 trailing-inventory and v4 stable-sharing options below address
parts of this requirement; accepted per-model selection, stable placement of all
citation metadata and full serving/cache qualification remain separate work. Only the
trusted envelope establishes authority; quoted source instructions remain data.
Clearly delimit reference data. Preserve complete programs, YAML blocks,
qualifications and related prerequisites together; nested source examples must not
become chat roles or new instructions. Do not summarize away executable artifacts
or repeatedly duplicate instructions. Stable source/component identifiers support
navigation and citation checks.

Separate original evidence from its model-facing display. A reviewed model design
may compact standalone strict JSON examples only when decoded values and embedded
program strings remain identical. Keep Python, YAML and prose literal; preserve
ambiguous JSON (duplicate keys, nonfinite values and precision-sensitive numeric
lexemes) unchanged. Retain original excerpts, source locations and quote resolution
against original bytes. Pin a display manifest to the generation with original,
display and rendered-card hashes; reject changed mappings rather than rewriting
evidence fingerprints. Verify transformations with real teaching artifacts and
negative examples. Display hashes establish integrity, not semantic approval.

Generate a compact task-boundary source/section index from actual selected headings
and exact citation IDs. Navigation labels never replace evidence identities. Do not
embed evaluation questions, expected answers or candidate feedback in that index.
Include a short general artifact audit scoped to five distinct artifact kinds:
host embedding, preload definition, invocation body, Recipe constructor and prompt
review. Host setup examples are embedding references, not class-22 entry points.
Preload definitions export typed functions without running them; invocation bodies
call the approved entry point and capture its result. Historical standalone teaching
programs do not override preload authoring rules. Trace absent/null/type/boundary
branches and typed returns against the actual artifact contract; verify
Recipe prerequisites in the emitted artifact rather than only its summary. Keep
this model-specific layout within the rendered context plus workspace reserve;
reject overflow without dropping mandatory evidence. Measure citation support,
first-pass emitted artifacts and transfer behavior before accepting a redesign.

Audit optional model-derived plans as source-anchored navigation against the entire
original trusted contract, not as executable branch specifications. Exact selected
quotes prove provenance only; they may omit requirements or misclassify an artifact.
Derive and check every branch from the original contract, including missing-only
defaults; an intermediate plan cannot cancel or invent them. Explicitly trace a valid path
through reject-on-failure loops and verify their success-state assignment. Count
distinct final values for cardinality/uniqueness requirements. For binary source
questions, the boolean must answer the actual proposition, including negation,
and agree with the explanation. Locate support in the full original evidence
before copying an ID; an index heading is navigation, not supporting evidence.

Teaching must transfer beyond a fixture: presence before value access, missing
versus null, exact booleans versus integers, recursive container/item constraints,
numeric boundaries, defaults only on missing inputs, complete invalid results,
helper invocation and module-level result assignment for invocation bodies,
effect-free preload exports and typed returns, typed step handoff, effects
and supported proposal transport. Pair positive complete examples with scoped
negative contrasts. Validate whole artifacts and behavior against independently
specified contracts; AST/schema success alone does not establish meaning.

Pin teacher manifests, validator code and transitive checker/schema dependencies,
execution environments and verification receipts. Reject drift until affected
examples are revalidated. Developer-edited examples require review; compiler output
must not turn model-generated explanations into official sources. Frozen evaluation
fixtures/oracles and failed-draft feedback never enter the reusable reference.
Feedback belongs only to that candidate's volatile review task. Keep first-pass,
bounded-repair and fresh/held-out results separate.

Concurrent component or documentation edits never justify changing expected hashes
inside compiler code, weakening integrity checks, reverting another author's work
or rewriting a published bundle. Capture a new coherent source snapshot, review
changed whole-unit selections, rerun affected teaching checks and compile a new
generation. Keep the previous sources, receipts, failed attempts and experiments.
Content digests belong to versioned source/policy/evidence manifests; the compiler
checks those records. Compare the build's pinned catalogue/document generation
with current desired inputs before activation and show changed sources as stale.
An in-flight experiment remains tied to its original snapshot; do not redeploy a
different prefix midway and combine its results into the same accuracy score.

Select content by required coverage and measured transfer, not “fill remaining
tokens.” Preserve the large prefix and full source store; admit optional whole
units only under a reviewed deterministic policy after mandatory units fit. If
mandatory knowledge cannot fit, fail with exact coverage/capacity evidence rather
than choosing lossy slices. A fully seeded base library may require a larger model.

### 5.2 Prefix-Maker reference and versioned design workflow

Introduce `prefix-maker` as the sixth seeded knowledge profile, independently
generated through the same model-free compiler and evidence gates. Register its
source package, procedure and `prefix-maker-compiler.py` entry point;
reuse the shared engine rather than another rendering implementation. Bootstrap
uses a reviewed shipped design and pinned local artifacts. Generating Prefix-Maker
must not require Prefix-Maker, a provider call or a pre-existing fresh prefix.

Its knowledge package includes authoritative, versioned model/provider/tokenizer/
chat-template and cache specifications; the four BrassClaw component guides and
supported profile/compiler interfaces; evidence-preserving source selection,
canonical formatting, navigation, code/YAML/JSON integrity, authority boundaries,
context/output capacity and stable leading-token construction. Include measured
lessons from Ornith and section 9.1: INT4 weights versus attention/recurrent dtypes,
full/linear attention groups, physical/logical pages, recurrent checkpoints, native/
external cache restoration, compatible namespaces, injection ownership and cache
qualification. Distinguish measured configurations from general principles; do
not apply Ornith geometry to an unknown model or claim a cache trains the model.

Teach complete general design examples and their validation receipts, including
coverage/capacity failures, contradictory sources and source-versus-instruction
boundaries. Apply the same freshness, licensing, complete-unit and held-out quality
rules as other profiles. Benchmark answers, secrets and conversations are not
reusable reference sources. Public/provider documentation refresh is a separately
recorded source job, never uncontrolled browsing during compilation.

Define two Tier-1 Recipes using reusable discovery/export/provider/proposal-store/
validation usages: Redesign an existing profile and Create a new profile. Record
their real component UUIDs, typed contracts, ten-plus intent examples and positive/
negative routing cases during implementation; explanatory steps here are not
persisted Recipe JSON. Each independent Tool/provider call is a separate usage.
Use the accepted DB-only provider gateway and actual sink contracts; introduce no
special Rust workflow executor or guessed API. The design provider may differ from
the target model: pin both identities and capabilities explicitly.

A Redesign task updates both design and knowledge: collect the current eligible
approved component catalogue, release/architecture documents and applicable model/
cache evidence under the configured refresh/review policy. Preserve newly collected
unreviewed sources in quarantine and report the prerequisite rather than treating
them as current authoritative knowledge. Pin the coherent current snapshot after
this stage; do not merely reformat an old generation or wait for a later Generate
click to incorporate the new information. Preserve the old generation as the
comparison baseline and return a complete updated candidate as the task result.

A design task pins an approved fresh Prefix-Maker generation/composite, current
profile/design/generation, source/evidence manifests, requested goals, hard coverage
requirements and target metadata. For new profiles the existing-profile reference
is absent explicitly. Compose one trusted envelope with Prefix-Maker exactly once;
the prefix being reviewed is delimited untrusted input, not a second system persona.
Check the full request capacity first. If it cannot fit, use only an explicitly
reviewed multi-stage inspection contract retaining complete evidence and coverage,
or return Capacity blocked; never silently slice the reviewed prefix. Unknown
architecture or unsupported adapter behavior requires evidence/setup, not invention.
Use the existing host secret/data handling before export to the design provider;
credentials and unrelated conversations never enter the review packet. User goals
and imported content remain typed task data, not Python source or trusted authority.

Provider output is a structured draft design: purpose/consumer, proposed source
registrations, coverage requirements, whole-unit selection and canonical ordering,
section/rendering rules, teaching/validation requirements, target compatibility,
expected tradeoffs and supported shared-engine options. Preserve the raw output,
rationale, exact request/response identities and diff against the existing design.
Provider suggestions about cache/service flags are advisory; this workflow never
patches packages, changes services or deploys inference infrastructure.

The model cannot install its output as arbitrary compiler Python, browser-supplied
paths or shell commands. Validate a versioned declarative design schema against
registered adapters and supported options. Missing engine/schema support is explicit
implementation work, not executable text or a silently ignored setting. If a design
requires new executable components, route them through separate supported authoring,
Q1/behavioral review/human Q2 and registration; do not treat the popup as approval.

Compile a complete quarantined candidate generation with the current pinned sources
and proposed model-specific design, including required base/domain composites; require
evidence preservation, coverage, formatting, capacity and reproducibility checks.
Optional provider evaluations are explicit child jobs with first-pass/repair/transfer
results and cache correctness separated. Show Unsupported, Capacity blocked or
Validation failed rather than claiming an upgrade merely because a model approved it.
Present old/new design and content diffs, omitted-unit reasons and receipts for human
review. Accept redesign records the new immutable model-specific design and publishes
its exact validated updated generation through a fenced, reconciled operation. Reuse
the candidate's verified artifacts, not a second unpinned compile. If content/design/
target changed meanwhile, report stale and require explicit rebuild/review; do not
accept yesterday's snapshot as current. Authored component changes still require Q1
and human Q2. Select the accepted design for future builds independently of production
prefix activation; the new generation is Generated, not automatically Effective.

Generate snapshots the applicable approved knowledge using the selected immutable
design. Identical design/target/source inputs reproduce identical output; new approved
components change information, not design. Only explicit accepted Redesign or a
reviewed design upgrade changes formatting/selection rules. Concurrent edits make
a proposal stale for acceptance; retain it for rebase/review, never overwrite a newer
design. Existing jobs/tasks retain their pinned revisions.

Prefix-Maker may itself have a Redesign button: use its previously approved fresh
generation to propose its successor. It cannot approve itself or use its unvalidated
successor as the review authority. If no usable predecessor exists, restore/build the
reviewed bootstrap reference first; do not fall back to an ungrounded model call.

### 5.3 Database-owned model specifications and model-specific designs

Prefix length, formatting and cache strategy are model-dependent. Persist model
specifications and design revisions in PostgreSQL through the final provider catalogue
and prefix stores, not browser state, environment variables or a second provider
registry. Reuse/extend the DB provider/model identity contract from simplified-v3 §10;
define migrations/DTOs explicitly where current storage lacks these records.

| Proposed record | Required contract |
| --- | --- |
| Model specification revision | Stable registered model identity plus exact weights/config/tokenizer/template identities, technical context/output limits, supported roles/tools/thinking/grammar, architecture and attention/recurrent-state facts with versioned evidence and Unknown where unavailable. Display name alone is not identity. |
| Target runtime/cache revision | DB provider/model binding, injection adapter, actual deployment/backend/KV and recurrent dtypes, group/page/chunk/checkpoint layout, effective limits and capability/qualification receipts. Model architecture and deployment-specific cache settings remain separate. |
| Model design revision | Immutable formatting/ordering/whole-unit selection, evidence-view transformation and audit/procedure revisions, teaching/rendering rules, explicit prefix-capacity/reserve contract and supported compiler-adapter revision/options. Pin consumer envelope and optional planner schemas, property order, fingerprints and decoder constraints separately from factual sources. Link exact model specification and compatible target runtime revisions, Prefix-Maker proposal/review evidence and schema/hash. |
| Profile/model design selection | Revision-checked mapping from profile and consumer to an accepted model design/coverage revision and compatible target; profile requirements remain independent of reusable model formatting defaults. |

A model can have multiple historical designs and profile-specific overrides; do not
make one global prefix token target apply to every profile/model. Shared defaults
are immutable reusable references with explicit overrides, not copied mutable blobs.
Record measured counts and full-envelope/output allowance; token budgets disabled
does not remove model context limits. A supported chunk-alignment strategy may be
part of a target-specific design, but changing runtime cache settings invalidates its
compatibility evidence without inventing different underlying model architecture.

Add a Tier-1 Generate model design Recipe: select an active registered model/target,
collect verified architecture/runtime evidence, use a fresh compatible Prefix-Maker
on the design provider, retain the proposed declarative design, validate supported
rendering/capacity options and trial examples, then expose it for acceptance. This
creates design records, not a prefix for every profile. Generate on a profile resolves
the accepted profile/model selection and compiles current approved content model-free.
Redesign additionally reviews that profile's knowledge and produces its complete
updated generation. Bootstrap model designs are reviewed shipped records and need
no provider call; unsupported new targets are explicit setup/design prerequisites.

Provider changes, Kohai model changes and Sempai provider/model changes resolve the
appropriate consumer's design and prefix generation independently. Prefix-Maker also
needs a generation compatible with its actual design-provider model, which may differ
from the model it is designing for. Never reuse an Ornith prefix/design solely because
another provider has the same friendly model name. Reuse exact compatible records
where evidence permits; rebuild rendering/capacity when tokenizer/template/effective
limits differ, and requalify caches when only runtime/cache identity differs.

On a provider/model edit, show affected consumers/profiles and accepted-compatible,
Needs generation, Needs model design or Incompatible states. An explicit confirmed
switch workflow may queue model-free regeneration for accepted designs; do not hide
provider-assisted redesign calls inside Generate or provider settings save. If no
compatible prefix is ready, keep the previous working selection until a staged switch
commits, or expose the newly selected target as blocked under its explicit lifecycle.
Never silently serve the old incompatible prefix, downgrade to a tiny fallback or
disable deterministic Tier-0/management paths. Unsupported architecture, rendering options or decoder contracts require adapter
implementation and qualification before a design can be accepted. A provider-produced
design cannot add compiler features or silently ignore unknown options. The current
Ornith-specific adapter is not evidence of general multi-model compilation.
New tasks use revision-checked selected
targets/designs; retained tasks keep pinned references subject to the provider's live
availability/policy contract. Preserve old model designs and generation history.

## 6. Installation-time DB seeding: preferred combined approach

Use the database as the installed source catalogue, with a shipped reproducible
reference package as its seed input. The compiler consumes an immutable export;
installed systems do not require the developer checkout or public network access
to reconstruct the base prefix.

1. Release packaging produces a content-addressed source package containing the
   current document inventory, full text, release/commit identity, checksums,
   compiler/profile revisions, source classifications and licenses where needed.
   Existing first-party executable component seeding remains authoritative.
2. Add reference-source registration to `initialize_runtime_components`, after
   migrations and required component seeding, before integrity/readiness completes.
   Reuse `reborn_docus` and its supported interfaces for compatible document records;
   add reviewed schema support for release/provenance/immutable revisions rather
   than inventing fields that current constructors do not accept.
3. Keep system-seed integrity and human Q2 for authored changes distinct. Shipped
   system references are checked against their release manifest; user imports/
   edits enter the supported validation path. Merely transferring a document into
   a DB or executing an upsert does not make it approved evidence.
4. Seeding is idempotent and preserves operator edits/older approved revisions.
   Upgrades create new revisions and atomically select eligible source revisions
   under the applicable release-integrity/approval contract. They invalidate affected
   prefix dependencies; they do not activate new compiled prefixes or overwrite a
   body used by a running task. Never seed generated
   summaries back as authoritative originals.
5. Register the six seeded profile definitions and compiler artifacts at installation.
   Seed reviewed model specifications/design defaults only for supported pinned
   targets with provenance; do not guess architecture for arbitrary provider names.
   Preserve operator model-design revisions, accepted bindings and history on upgrade.
   Optionally import an existing verified domain generation. External domain source
   fetches and expensive generation are operator jobs, not boot-time network/GPU
   prerequisites. A missing domain bundle is visible as Not generated.
   Seed Sempai source/procedure metadata with its own checksum manifest. An optional
   imported reviewer generation must satisfy the same source/freshness checks;
   presence of reference text does not activate authored components or Q2 evidence.
6. Base-prefix generation follows successful boot prerequisites through the accepted
   model-free generation Recipe. An initial background job or packaged compatible generation
   may be used, with explicit readiness state. Do not manufacture a Monty global
   lifecycle in a WebUI constructor; honor simplified-v3 Phase 3a's real handshake.

Separate source catalogue, compiler artifacts and compiled prefix storage. Storing
complete originals does not eliminate the need to persist assembled prefix text.
Persisting a prefix does not eliminate raw evidence or installation seeds.

## 7. Persistence and generation lifecycle

Design forward migrations after the actual migration head. Do not rewrite V063,
V085, V087 or V088 on an installed system. The following are proposed contracts,
not existing tables/API fields:

| Record | Required content |
| --- | --- |
| Prefix profile | Stable profile ID, display metadata, registered compiler/version/checksum, source/coverage policy, model targets and supported validation adapters. |
| Design revision/proposal | Model-specific declarative design/schema revision and checksum, profile/consumer bindings, predecessor, pinned Prefix-Maker/design-provider/model-specification/source/target references, raw proposal, diff, exact updated candidate generation and validation/acceptance evidence. Draft proposals never become accepted designs by a status label alone. |
| Model specification/design selection | DB-owned versioned records and mappings from section 5.3, with explicit compatibility/Unknown states and retained revisions; no browser-only model-formatting configuration. |
| Source registration/observation | Versioned profile-source identities and applicability/review state plus separate append-only availability/content-check receipts from section 8.1. Generation manifests pin used revisions; transient observations do not alter source or generation identity. |
| Prefix generation | Content-addressed ID, profile ID, compiler/input snapshot identities, full bundle text, evidence cards, immutable source/dependency manifest, model/tokenizer/template hashes, exact token counts and quality results. |
| Generation job | Durable request/run ID, profile, pinned input references, state/stage, timestamps, logical step-invocation references, execution attempts, cancellation/fencing, checkpoints/artifacts and classified errors. |
| Active selection | Provider/model/consumer binding to one immutable generation or explicitly compiled composite; desired/effective state and revision for compare-and-swap. |
| Cache observation | Provider process/model identity, generation/token-prefix hash, runtime/cache-layout fingerprint and salt policy, expected-content checks, native/extra-external hits, warm/reload measurements and time; never confused with generation success. |

Migrate existing `reborn_basic_prompt_store` bundles as legacy base-prompt generations.
Keep compatibility reads until both Kohai and Sempai use the new resolver. A legacy
bundle without evidence/tokenizer provenance is labelled Legacy / Unverified;
import does not synthesize missing verification. The old scope-only uniqueness
constraint cannot distinguish named profile identities: add the actual name/selection
dimension or introduce generation storage with a clearly defined compatibility
pointer. Do not assert additive rows already solve this.

Job states: queued, snapshotting, collecting, compiling, validating, completed,
failed or cancelled; represent prerequisite/capacity blocking explicitly with a
reason and resumable dependencies rather than a misleading queued state. Generate
operations may contain parent and composite child jobs. Activation and cache
states are separate. Persist real stage
progress and counts; report a percentage only with a known denominator. A browser
disconnect does not cancel admitted work. Deduplicate repeated submissions and
serialize conflicting profile/target mutations, while allowing independent builds
within configured resources.

Job submission deduplication is distinct from Tool-invocation retry safety. Reuse
the runtime's durable invocation/effect records, not a second permanent turn queue
or persisted BuildInstruction. Each logical step invocation retains its positive
dispatch count, including the initial dispatch, across waits, reclaims, process
restarts and replacement attempts. Before effects, persist dispatch intent, pinned
workflow/implementation references, required argument identity and any stable
deduplication key; failed persistence prevents dispatch. Persist completion/effect
status against that invocation before advancing the Recipe.

Apply skills.md's exact failure contract and simplified-v3 Phase 0a items 9–10.
Retry only explicitly eligible outcomes with trusted read-only or tested durable
deduplication evidence. Reuse the same key and arguments throughout a continuation;
a new worker attempt does not reset the logical invocation count. A timeout or
missing result is unknown completion, never proof of no effect. Recipes reconcile
compiler artifacts/publication status through existing primitives before deciding
whether a permitted retry is safe; unresolved effects stop with a classified error.
Recheck cancellation, claim fencing, live Tool policy, auth and resource constraints
before every permitted retry. Never rerun confirmed effects because output checking,
reply delivery or a later step failed. No DB-plus-provider/filesystem atomicity is
assumed; define durable reconciliation for effects outside the DB transaction.

Publish only after all required evidence/coverage/token checks pass. Publication is
atomic; a separate selection change is independently atomic and revision-checked.
Publication does not change active selections. Failed regeneration leaves the previous immutable
generation intact. A completed publish is reused after downstream reply/warm failure;
never rerun its effects because an HTTP response was lost. Cancellation, policy,
attempt fencing and durable retry counts apply throughout. Artifact retention follows
active selections, jobs and resumable task references, not “keep newest only”.

Staleness is dependency-aware: relevant approved component activation, release-doc,
source, compiler, tokenizer or template change marks only affected generations.
Replace best-effort staleness updates with durable invalidation/reconciliation.
Expose separate Source outdated, Incompatible and Revoked states. Retain last-good
generations for rollback and tasks that already pinned them; retention does not make
an outdated generation eligible for a new task. New tasks require a generation
matching the current applicable source/selection revision. Rebuild and queue affected
model work, or return an explicit Prefix rebuilding / blocked result. Recheck
freshness and pin the generation atomically against that revision so an activation
race cannot admit new work with stale context. Existing continuations retain their
pinned generation after ordinary replacement; explicit revocation/incompatibility
blocks affected dispatch with a classified error. Make this policy explicit and
never silently substitute the current tiny fallback. This follows the new-task
freshness requirement in simplified-v3's acceptance matrix.

## 8. WebUI and API changes

Extend the existing Prefix tab rather than its frontend-only list. All six seeded profiles and registered custom profiles
appear even before their first build. Display a useful domain description, generated
and active generation, source freshness, build status, last build duration,
rendered tokens, coverage gaps, validation status and per-provider cache status.
Keep ordinary UI copy about knowledge and readiness; expose hashes/geometry and
raw manifests in an expandable details area.

Before Generate, show the resolved model/design revision, compiler adapter and source
snapshot, requested/effective prefix capacity, workspace/output reserve, coverage
policy and evidence-display mode. Distinguish stored defaults from experimental
settings. Unsupported options, stale selection policies and incompatible ordered
consumer grammars block readiness with their actual prerequisite; do not silently
ignore them or ask the operator to repair runtime services. The UI resolves DB-owned
inputs, rather than copying the standalone shell environment from §4.2.

Actions: Generate / Regenerate for each profile; View job / Cancel while running;
View sources and validation; Activate a chosen compatible generation; Warm /
Verify cache when the provider supports it; Roll back to a retained generation.
Generating a prefix must not automatically replace an unrelated provider's active
domain. Do not mark “fresh” optimistically from a job admission response.

Place a button labelled **Redesign** immediately beside Generate / Regenerate on
every seeded/custom profile row. Before admission, open an accessible confirmation
popup naming the profile/current design, design provider/model and target, permitted
source data sent to that provider, requested goal and the expected outcome: refreshed
knowledge plus a model-specific design and complete updated candidate generation for
review, with no active prefix changed. Offer Cancel and Confirm redesign;
Escape/cancel makes no job or provider call. Revalidate prerequisites and exact
revisions server-side on confirmation. If they changed, require confirmation of the
updated request. Persist confirmed request identity/idempotency before dispatch so
double clicks, reconnects or recovery do not create duplicate confirmed effects.
This is the requested UX confirmation, not a per-Tool authorization lease; current
global policy, external authentication and technical limits still apply independently.

Redesign shows content/architecture refresh, durable progress, raw design/content diff,
complete candidate validation and a separate Accept redesign action. Failed/cancelled/
rejected proposals preserve the selected design, generated artifacts and active
selection. Acceptance records the new model-specific design and publishes the exact
updated candidate generation; no extra Generate click is required to obtain it.
Activation remains separate. Source-only refresh does not change design; Redesign
includes current knowledge refresh, while Regenerate never asks a model to rewrite
the design. Missing provider/fresh Prefix-Maker/target evidence disables Redesign with
an actionable explanation while offline Generate remains available.

Add a **Create Prefix** tab within this same settings section, alongside the existing
profile listing. Collect display name, purpose, consumer, target and registered source
references/authorized import requests, mandatory coverage and optional design goals;
allow selection of the design provider. Allocate stable profile identity server-side;
names are not paths or compiler entry points. Show a confirmation summary before the
provider-assisted Create design job. Validate duplicate/conflicting names, unsupported
consumers/adapters and source access before dispatch. Draft profiles are visibly Draft,
not eligible serving selections. On validated human acceptance, register the custom
profile/design, then expose it in the same list with Generate and Redesign. Registration,
generation and activation remain distinct. Seed upgrades cannot overwrite custom
profiles/designs; removed drafts cannot break retained job/artifact references.

Add a **Model Designs** area/tab in the same Prefix settings section. Its selector
lists active DB-registered provider/model targets, explicitly identifying the model
being designed for and the separate provider doing the design. **Generate model
design** opens a confirmation summary, then admits the section 5.3 Tier-1 Recipe.
List already generated model designs, including retained designs for inactive models,
with model/provider identity, specification/design revision, formatting summary,
prefix-size/reserve rules, evidence provenance, validation/acceptance status, compatible
targets, profile bindings and affected/stale generations. Show unknown architecture
and missing prerequisites rather than guessing cache features. Details expose hashes
and geometry; ordinary copy explains what will fit and which profiles use the design.

Offer View/Review, Accept model design and explicit profile-binding controls; proposals
do not overwrite accepted records. Review the impact of selecting another design on
Kohai, Sempai and Prefix-Maker separately, and offer explicit Generate/Redesign actions
for affected profiles. Provider/model changes surface compatibility and missing-build
states here. Merely selecting a model in the form makes no provider call, prefix build,
deployment change or active selection mutation. The list comes from persisted records
and survives reload/restart, including historical designs and their receipts.

The primary Generate action uses a registered saved build specification: profile,
consumer, optional target revision, accepted source-refresh policy and reviewed
coverage/teaching policy. Operators do not compose shell flags or choose packing
algorithms. Missing choices are explicit setup prerequisites. Snapshot-only offline
building remains available without a provider. Advanced parent/composite actions
are not mandatory hidden steps for normal target-aware generation.

Show one operation timeline across parent/dependency/compile/validation stages with
exact blocked reasons and counts. Distinguish source refresh/review from compilation:
use an eligible pinned package or an explicitly authorized refresh policy; changed
unreviewed sources are quarantined, not auto-approved. Unchanged inputs deduplicate
to the same content-addressed generation while retaining the requested operation.
Explicit refresh/rebuild creates new immutable inputs, never overwrites evidence.

Details expose coverage and omitted units/reasons, source versions, actual capacity,
teacher checks, first-pass/repair scores, injection owner and cache qualification.
Show Cache unavailable, Unverified and Cache correctness failed distinctly. Failed
enabled caching is a serving prerequisite failure, not missing telemetry. Citation/
configuration/review support is per adapter; no universal “expert” badge follows
from compilation or a warm timestamp.

Retain the current API family. Proposed extensions, subject to final DTO review:

| Operation | Proposed route/behavior |
| --- | --- |
| List | Existing `GET /api/webchat/v2/prefixes`, now returning seeded/custom profiles plus current job/build/selection state. |
| Generate/regenerate | Existing `POST /api/webchat/v2/prefixes/{name}/regenerate`, admit work and return a durable job reference (`202`); one operation handles first and later builds. |
| Redesign | Proposed `POST /api/webchat/v2/prefixes/{name}/redesign-jobs`: confirmed typed request with exact existing design/generation, registered design-provider/target references and goals; returns durable job reference (`202`). |
| Create profile draft/design | Proposed `POST /api/webchat/v2/prefix-profile-drafts`: validated form and confirmed provider-assisted design request; allocates draft identity and returns its durable job reference. |
| Review/accept design | Proposed design-proposal detail and explicit acceptance mutations under the profile/draft family; require exact proposal/checksum, predecessor revision and validation/review evidence. No acceptance by arbitrary JSON overwrite. |
| Model design catalogue/jobs | Proposed `GET /api/webchat/v2/prefix-model-designs` and `POST /api/webchat/v2/prefix-model-design-jobs`; read persisted model/design records and admit confirmed design jobs using registered model/target/design-provider references. |
| Model design acceptance/binding | Proposed detail/acceptance and profile-model binding mutations under that API family; exact model/design/schema revisions, compatibility evidence and compare-and-swap, never browser-authored runtime facts. |
| Source inventory/registration | Proposed `GET /api/webchat/v2/prefixes/{name}/sources` and `POST /api/webchat/v2/prefixes/{name}/sources`; return generation provenance plus current registered sources, or save a validated new source registration. |
| Source availability check | Proposed `POST /api/webchat/v2/prefixes/{name}/source-check-jobs`; admit a bounded check of pinned registered source identities and return job/progress/result references, rather than performing network I/O in a GET handler. |
| Observe/cancel | Implemented `GET /api/webchat/v2/prefixes/{name}/jobs/{job}` projects the existing run/admission/effect records. Use its opaque `thread_id` and `job_id` with the existing run/event/cancel APIs; do not add another prefix-job lifecycle or cancellation store. |
| Offline model/design measurement | Implemented `POST /api/webchat/v2/prefixes/{name}/jobs/{job}` admits `operation=measure` through the same run owner and returns the current job projection. Explicit retained generation/proposal/artifact IDs and probe/limit assumptions; no acceptance or activation. |
| Generation details | `GET /api/webchat/v2/prefixes/{name}/generations/{id}` with manifest, source and quality projection. |
| Activation/warm | Explicit mutation operations under the same profile family with registered target identity and exact generation, not arbitrary host paths or shell commands. |

Changing synchronous regeneration to job admission requires updating facade DTOs,
all callers and contract tests together. Provide a documented compatibility path if
other clients depend on the old completion response. Extend the existing
`InterceptorConfigService`/`RebornServicesApi` seams, or extract a prefix port behind
that facade if needed; do not build a competing WebUI-only compiler service.

Thread private worker/store/provider handles through `RebornRuntimeInput`, runtime
factories and `build_webui_services`. Handlers remain thin facade calls. Update route
descriptors, router mounts, sanitized errors, API functions, hooks, tab components,
translations and contract tests as one vertical change. The server owns profile IDs,
compiler paths and source packs; the browser selects registered IDs and validated
settings. Keep authentication, origin/body limits and technical resource enforcement
at their existing boundaries without adding legacy feature-role authorization.

### 8.1 Sources button, source inventory and availability checks

Add a button labelled **Sources** beside Generate / Regenerate in each seeded/custom
prefix row, alongside Redesign. Clicking opens a profile-specific **Sources** tab in
the same settings section; preserve the selected profile/generation and provide a
clear return to the prefix list. This is a source-management view, not another prefix
compiler. Implement it through the existing facade, composition and ingress seams.

Show every source actually used by the displayed generation, including inherited
base/composite sources, official web documents, qualified forum/local findings,
DB component records, release documents and teaching packages. Derive this inventory
from the immutable source/evidence manifest, not a reconstructed list of URLs. Also
show current source registrations, newly saved sources and omissions separately, so
“used in this generation” cannot be confused with “available for the next build.”
Before the first generation, show the registered source catalogue with Not yet used.

Each row shows title/type, URL or safe registered reference, source/version identity,
availability indicator, last check time/result, archived snapshot availability,
review/disposition and usage in the selected generation. Use **green Available**
when the declared source/content can actually be retrieved/read through its adapter,
and **red Unavailable** for a completed check that cannot retrieve/read it. Add
neutral Checking / Not checked / Check error states with text and accessible labels;
no color-only meaning and no false red during an unfinished check. Explain reasons
such as Missing, Authentication required, Timeout or Rate limited. Availability,
content changed and knowledge approval are separate fields: green never means
authoritative, unchanged or approved. A dead upstream URL does not delete an intact
archived source or change a historical generation's provenance.

On tab opening, render the inventory immediately and admit a bounded, model-free
source-check Recipe against the pinned source registrations. Stream per-source
results; offer Check again. Coalesce repeated opens/concurrent tabs into an existing
compatible check job, enforce concurrency/time/byte/request limits and show the age
of previous observations while rechecking. Closing the tab stops UI subscriptions,
not a separately admitted durable job. No continuous background crawl is introduced.

For web sources, use a short content-aware fetch through the existing registered
HTTP/fetch primitive: bounded redirects and response size, expected media/extraction
adapter, and sufficient content checks to detect empty pages, login/error pages or
soft 404s. HEAD may precede GET where useful; ping/DNS/HTTP 200 alone cannot establish
that the referenced document still exists. A range/snippet can establish a limited
availability observation but cannot certify full-content identity. Compare complete
content hashes only when the supported bounded fetch actually retrieves the full
artifact; otherwise report Content identity not checked. Authentication uses existing
host secret resolution, never URL credentials or secrets stored in manifests.

For DB/release/local artifact sources, resolve the registered identity/revision and
verify read/access and retained artifact integrity without web requests or arbitrary
browser-supplied filesystem access. All actual checks obey current global Tool policy,
network/secret constraints and technical limits, including redirect destinations and
private-network targets under the existing supported policy. URLs remain typed data;
the check does not execute source content or scripts, crawl unrelated links or create
a provider call. Reuse a deterministic non-shell fetch/check adapter; shell variants
remain Tier 1 and cannot be the sole model-free path.

Provide a text field labelled **Add source** and a **Save source** button in the tab.
Accept an HTTP(S) URL or a supported registered document/component reference; offer
source type/title/applicability details where needed. Validate and canonicalize through
the supported adapter without stripping meaningful version/query identifiers. Reject
unsupported schemes, embedded credentials and duplicate conflicting registrations;
show duplicate existing sources instead of silently adding another row. Saving persists
an immutable/versioned profile-source registration in PostgreSQL, with stable identity,
origin, applicability, intended required/optional coverage and review status. It is
not an automatic fetch approval, redesign, build or activation. A subsequent bounded
availability check may run through the same admitted check path.

A newly saved source is **Pending collection/review**, visibly not yet used. The next
confirmed Redesign collects it under the registered refresh policy, preserves original
bytes/provenance and checks relevance, quality, licensing and compatibility before
incorporation into the candidate. Generate/Regenerate may include it once an eligible
reviewed snapshot exists and the accepted design's registered source/coverage rules
permit it; adding a URL must not invoke an LLM or silently change those rules. If new
coverage needs a design change, show Needs redesign. If required content cannot fit,
report Capacity blocked; optional exclusions need explicit recorded reasons. Respect
trusted bundled source provenance versus post-installation edits; a source label or
green check bypasses neither authored validation nor component Q1/human Q2.

Keep availability observations separate from source revisions and generation identity:
timestamps and transient network failures never enter stable prefix text. Checks are
read-only diagnostics and cannot rewrite approved snapshots, delete sources, mark an
unchanged generation outdated solely because of a timeout, or invalidate active tasks.
Reviewed source-content/selection changes follow dependency invalidation normally.
Preserve historical registrations/snapshots and pin new jobs to a coherent selection;
sources saved mid-build apply only to a subsequent admitted build.

Implement source inventory, registration, collection and checking as separate reusable
usages in Recipes with typed results and retained identities; independent Tool calls
remain separate steps. Define real store/adapter contracts before authoring component
rows. Record positive/negative intent cases and exact errors/retries; the UI opening
calls the admitted workflow through the facade, not a second browser/server executor.

## 9. Serving, large-prefix capacity and hybrid cache

Provider definitions and active configuration come exclusively from PostgreSQL
(`brassclaw_llm_providers`) through the existing `PgProviderRepo` or a neutral port,
as required by simplified-v3 §10. Resolve provider identity, protocol, model, URL,
context capacity and active selection from that DB contract; prefix profile/model
metadata describes compatibility, not a second provider registry. No embedded
provider list, recurring Rust provider seeder, file catalogue or Env override/fallback
may supply production targets. Offline pinned tokenizer/model artifacts remain
compiler inputs, not an alternative provider configuration source. DB bootstrap and
Secret-Broker resolution remain separate; secrets never enter source manifests.

Use the existing revision-aware reload contract and show desired/effective state.
Provider deactivation/removal transactionally resolves affected active selections
to an explicitly chosen alternative or Unconfigured. Future dispatch, including a
continuation, must not use an inactive/hidden provider merely because its prefix is
retained. Treat already-started calls under the provider's defined completion/cancel
contract without replay. Reactivation is explicit, validates adapter/model/auth
compatibility and never silently restores previous selections. A DB error is a
technical error; absent configuration is Unconfigured. Neither triggers fallback.
Operator management and model-independent Tier-0 tasks remain available.

Choose a serving mode per registered provider target:

- **Client-owned prefix:** BrassClaw sends the selected immutable context before
  role-specific instructions/history/task. This is the default for per-request
  domain selection on a shared endpoint. The provider must not inject another
  large reference automatically.
- **Server-owned prefix:** a dedicated or otherwise verified endpoint injects the
  exact selected generation through its registered template. BrassClaw sends no
  duplicate prefix and verifies the effective token prefix. A setting in the UI
  alone cannot change a provider's automatic injection behavior.

Server-owned mode additionally requires an enforceable generation contract: either
route to an immutable generation-specific endpoint retained for its pinned tasks,
or use a provider API that atomically checks the requested generation and renders
that exact prefix in the completion operation. Retain the endpoint/template binding
through continuations and retries; a restart must restore the same binding or fail
closed. A separate `/tokenize` request is diagnostic evidence, not protection
against a template change between verification and completion. Mutable shared
endpoints without atomic enforcement are ineligible for server-owned pinning; use
client-owned injection with automatic reference injection disabled, or block the
target. Do not alter deployment services automatically to establish compatibility.

Rollback uses the same freshness, compatibility and generation-enforcement checks
as activation. A retained older generation is not automatically eligible for new
tasks; rebuild it against current applicable sources when those checks fail.

Pin the generation/injection mode with each admitted model task and continuation.
Do not reread the active pointer during that task. Provider/persona/reviewer schemas
and volatile state remain distinct from the common evidence corpus. Route-specific
Kohai and Sempai prefixes/warm states are independent. A deterministic Tier-0 task
still needs no model call merely because a prefix exists.

For BrassClaw Kohai/Sempai consumers, domain/reviewer selection selects an explicitly compiled
base-plus-domain composite; it does not replace the required BrassClaw base knowledge.
The seeded and registered custom UI profiles remain independently buildable source generations. Composite
records pin the exact base/domain generations, rendering policy and token manifest;
the active selection points to the composite. Standalone domain generations remain
usable by separately registered domain-only consumers. If a required composite
cannot fit, leave it Capacity blocked rather than activating a domain-only substitute.

### Composite generation workflow

Add a reusable composite-building Recipe with typed inputs: exact base/domain
generation IDs, registered target ID and registered rendering/coverage policy ID.
Resolve immutable parent source exports/evidence through existing store primitives,
pin the DB target revision and local tokenizer/template artifacts, run the shared
compiler's composite adapter, validate, then publish a distinct immutable composite.
Use the same durable job, invocation, cancellation and artifact contracts as other
builds. Composite building is model-free and uses the accepted non-shell worker
path; activation remains a separate operation after successful publication.

The adapter renders source units from pinned parent exports under one envelope;
it never concatenates two rendered chat templates. Preserve original evidence IDs
and source/revision/range identities. Deduplicate identical cards only when their
identity and payload agree; conflicting payloads for one ID fail validation.
Record parent generations, ordered units, coverage policy, required units and any
explicitly permitted partition in the composite manifest. Preserve all required
base content and declared domain coverage; do not silently drop units to fit.
Recompute the composite template, full context/output capacity and leading token
fingerprints rather than reusing either parent's cache geometry. Capacity failure
publishes no activatable composite and leaves parents/active selections intact.

Propose `POST /api/webchat/v2/prefixes/{name}/composite-jobs` for a domain or Sempai reviewer profile,
returning `202` and the existing durable job reference. The server validates exact
parent IDs, target/policy IDs and profile relationships; it accepts no paths/code.
Use the existing job status/cancel routes and generation-detail projections, extended
to show composite parents. Keep top-level rows for seeded and registered custom profiles; show target-specific
composites beneath their domain/reviewer row with Build composite / Rebuild, job progress,
Capacity blocked and Activate state. Parent generation alone is labelled Generated,
not Ready for BrassClaw activation. Normal target-aware Generate orchestrates this
Recipe with required parent tasks through the same job service; the explicit
composite endpoint supports advanced exact-parent builds. Never implicitly rebuild
within Activate.

Parent source/selection changes invalidate dependent composites for new tasks.
Rebuild explicitly pins the newest eligible parents under one coherent selection;
if a parent is missing/outdated, report that dependency and build it first through
the normal Recipe. Already running composite jobs retain their inputs; a completed
outdated result remains historical and cannot activate. Dedupe on exact parents,
target/tokenizer/template revision and policy/compiler identity. Preserve existing
task references and independently usable domain-only generations.

Keep a stable base spine before a selected domain branch where possible. Do not
concatenate complete 100k bundles for every profile: the verified example Ornith endpoint has a
131072-token technical context limit. Register compiled composite profiles or
model-compatible selections with explicit manifests and measured capacity. Store
the entire source library regardless; if required complete units cannot fit,
report Capacity blocked and offer a different model/context or an explicitly
reviewed partition. Never silently truncate code/bindings, drop required guides,
hide a component cap or pad the prefix with low-quality repetition.

Production policy follows `token_budgets_enabled=false` by default: no hidden
retrieval/history/knowledge caps. Actual model context/output capacity remains a
technical limit. Distinguish the current compiler's example 100000-token packing
target and explicit operator settings from disabled global token budgets. Reserve
workspace based on the real serving contract and selected model; do not install a
universal hardcoded 100k/31k policy in every provider.
Count the actual complete serving envelope, including tools, role/template overhead,
current history/user input and requested output allowance, before each dispatch.
Build-time reserve is an explicit target contract, not a guarantee that every future
conversation fits. Oversized requests return a specific capacity outcome; no hidden
history truncation or discarded mandatory knowledge compensates for overflow.

Keep original tokenizer and chat-template artifacts immutable. Measure leading token
IDs across thinking, tool and system-message variants. Stable text hashes alone do
not prove shared token identity. Dynamic timestamps, job IDs, source retrieval times
and metrics stay in manifests rather than the leading reference text. Stable source
identity/content stays in the evidence rendering.

The current Ornith compiler profile distinguishes 24 linear-attention layers and
8 full-attention layers, 4-bit weight configuration, attention KV dtype and
recurrent-state dtype. Read quantization configuration rather than inferring its
method from the model name (the retained artifact reports compressed-tensors). Preserve that distinction: INT4 weights do not mean INT4 KV or recurrent
state. Parameterize provider/model adapters rather than assuming every model has
this geometry. Derive hybrid block/chunk compatibility from the actual pinned
configuration and runtime. Treat previous 1056-token-block/99264-token reuse
receipts as observations for that deployment, not constants for all targets.

### 9.1 Runtime/cache qualification and stable namespaces

Compilation creates reference/context artifacts, not GPU attention pages or
recurrent state. Inference creates those caches; eviction/restart may remove them.
Caching is context reuse, not training or guaranteed knowledge persistence. A
compiled generation and qualified serving target have different lifecycles.

Qualify actual runtime/model/tokenizer/template and cache layout: vLLM/connector/
LMCache source or build hashes (including backports), attention backend, parallelism,
weights/config and relevant scale identity, KV/state dtypes, logical/physical block
mapping, group/chunk geometry, checkpoint mode and prefill scheduling. Associate
verified fingerprints with DB target revisions. Package versions are insufficient.
Relevant runtime changes invalidate target qualification; rendering changes
invalidate effective-token evidence. Renewing an observation does not require
rebuilding identical reference content.
Separate runtime compatibility identity from process lifetime: compatible restarts
retain the declared namespace, while invalidating process-bound warm observations.
PID/random engine IDs must not create a new namespace on every restart. Claim
restart/disk reuse only after a real supported restore passes content checks.

For enabled hybrid external caching, verify independent recurrent/full-attention
groups and compatible transfer code, not just chunk divisibility. Derive flags from
supported interfaces/effective metadata; absence of an explicit hybrid-manager flag
does not prove the manager is disabled. Sparse align-mode checkpoints exist at
scheduler boundaries, not every attention block. Larger prefill batches require a
separate measured experiment, never automatic adjustment by Generate.

For GDN/linear-attention hybrids, qualify behavior as well as cache-hit counts.
The inspected Ornith release has 24 linear-attention and eight full-attention
layers; the historically inspected deployment used a 1,056-token unified block.
Architecture and deployment block layout are separate evidence, not universal
defaults. Refresh runtime layout before applying those historical measurements. `--no-disable-hybrid-kv-cache-manager` keeps the
manager enabled but cannot prove recurrent-state restoration. Record effective
Mamba cache mode, attention/recurrent dtypes, page layout, group separation and
checkpoint granularity separately from INT4 weight quantization. Do not infer
an INT4 KV cache from AWQ weights. GDN cached/fresh generation is not necessarily
bit-identical; compare behavioral scores with fixed requests and decoding rather
than treating any token difference as corruption. Preserve cold, native, external
and restored-native outcomes independently. A sentinel restore is not a semantic
accuracy comparison. See the [LMCache hybrid guide](https://docs.lmcache.ai/mp/hybrid_models.html).

For an inspected block size N, an align-mode batch in [N, 2N) offers finer boundary
checkpoints; larger batches trade checkpoint density for prefill throughput when
supported group separation is active. Benchmark those modes under a fixed design
before selecting a model's accepted settings. Never apply another model's N,
force unsupported all-mode or enable speculative/sub-block features merely from
generic documentation. Preserve operator-owned deployment settings during prefix
generation; a configuration experiment needs separate retained runtime evidence.

For this hybrid, the linear layers retain recurrent summaries rather than an
independent attention key/value pair for every preceding token. Full-attention
layers still retain their KV history; cache capacity alone does not establish
reliable retrieval of every rule. Trial semantically grouped references, explicit
source/contract precedence and a compact terminal audit near the volatile task.
Keep these structures stable within an accepted design and preserve all selected
original evidence. Measure generalization and citation support, not only the
frozen score. Do not insert arbitrary padding or claim that placing a paragraph
on a 1,056-token boundary improves its meaning: physical checkpoint alignment
and semantic source organization are separate properties to qualify.

`cache_salt` only namespaces entries. Use a stable namespace under the target's
cache-isolation/runtime-layout contract; never create a salt per request or change
it between prime/planner/writer/repair calls within an experiment. Introduce no
tenant/feature-role registry for the single-instance operator. Keep incompatible
cache layouts separate; reconcile old objects through supported provider management.

Enforce generation selection independently through immutable injection and exact
tokens. Where safe shared-base reuse is intended, a compatible namespace may span
domain generations: token hashes distinguish divergent inputs. Salting every full
composite isolates experiments but prevents cross-domain reuse of the common base
spine. Record the tradeoff; never discard isolation to inflate hits. Salts/runtime
epochs/statistics do not belong inside model-visible reference text.

For observable local cache adapters, compare identical requests serially across
cold compute, native reuse, native eviction with verified external restore, then
native reuse of restored state. Keep prefix/template/messages/decoding fixed; retain
raw responses/errors, expected-content checks and request-local native, external
and **extra external** counts. LMCache total hits overlap native hits; extra hits
establish additional restoration. HTTP 200/native hits do not prove correct state.
Include unconstrained and constrained decoding and representative factual/behavioral
probes; a fixed JSON grammar alone can hide nonsensical logits.

The measured Ornith short warm probe ended within the final shared block, establishing
a static checkpoint before long histories. Apply this only where the resolved
checkpoint contract supports it; verify actual tokens and expected content. Priming
cannot certify corrupted native state solely by hits. Qualify while the shared
target is idle or through supported isolation; do not evict unrelated live requests
or claim load safety from a serial probe.

Measured 2026-10-07: LMCache 0.5.5/vLLM 0.31.0 omitted hybrid object separation and
its attention adapter matched only rank-five tensors. Ornith's rank-four buffers
used 32-token physical pages for 1,056-token logical blocks. Separation alone still
corrupted outputs. The relevant class from upstream
[PR 4253](https://github.com/LMCache/LMCache/pull/4253), alongside separation, passed
plain/JSON sentinels with three verified 97,152-token external restores. See the
[verification receipt](../../scripts/prefix/sempai-accuracy-20261007/cache-fix-verification.json)
and [source/layout review](../../scripts/prefix/sempai-accuracy-20261007/lmcache-4253/source-and-layout-review.json).
This qualifies that runtime; it proves neither arbitrary load, disk/restart
persistence nor model accuracy.

These layout/correctness checks precede production activation when the configured
path can serve external hits. Known-corrupt paths are ineligible, not Unverified;
fresh salts are not remediation. Requalify a repaired path or explicitly select an
independently accepted alternative cache mode. External caching remains optional:
providers not using/exposing it are not blocked by absent LMCache telemetry.
Deployments remain operator-owned; Generate never patches packages, changes Compose,
reloads systemd or restarts vLLM.
Qualification is a separate observable operation against an immutable candidate
using an accepted injection/isolation adapter; it must not require activating that
candidate first. A completed build stays Generated if qualification later fails,
with the target explicitly ineligible. Generate has no hidden model calls. Existing
runtime qualification may be reused only within its recorded compatibility scope;
candidate-specific token/injection evidence still names the exact generation.

Warm through the provider that will actually serve the selected prefix, using its
real template/tool envelope and a minimal normal completion when supported. Provider
adapters declare warm-request support, token inspection, native reuse telemetry and
external reload verification separately. Correct injection, capacity, freshness and
generation enforcement gate activation, alongside section 9.1's correctness gate
for enabled external cache paths. Missing telemetry alone does not block a valid
provider. Verify a second
request's reuse only when observable; otherwise report Cache reuse unverified and
disable unsupported verification actions without blocking valid inference. Token
inspection is also capability-specific: client-owned mode verifies exact outbound
content/manifest and uses accepted adapter/tokenizer capacity evidence; server-owned
mode still requires its enforceable completion-time generation contract. Do not
claim exact server token/cache observations when the provider does not expose them.
Record full-attention KV and recurrent-state support honestly where applicable.
LMCache external persistence is a separate optional adapter capability. Test reload
after native-cache eviction when claiming external reuse; a warm timestamp, HTTP
200 or native hit does not prove external recurrent-state storage. Do not replace
Compose, venvs or systemd units as part of this upgrade's default flow.

## 10. Citation and configuration validation in production

Reuse `prefix_response_validation.py` and its exact source identities, indexed
quotation copying, quotation validation and bounded repair behavior. Base-prompt
evidence additionally resolves document release/range and component UUID/revision/
field. Add adapters without breaking the existing domain generation format.

Carry the selected evidence manifest into actual Kohai/Sempai/gateway answer paths
and verify it matches the injected generation. Do not merely ship the helper script
and claim all answers are checked. Integrate with the existing message/tool/reviewer
contracts: the standalone `claims/configuration/missing_information` schema cannot
replace every tool-call or Sempai response schema indiscriminately.

Track provenance success separately from semantic support and runtime truth. Exact
quotes and basic word overlap can still support an incorrect interpretation or
reject a legitimate translation. Show that limitation and require appropriate
review. Production evaluation includes wrong-but-verbatim citations and misleading
source/version combinations, not just invented IDs.

Allow a model-specific design to select short readable citation handles if it
performs better than opaque hashes. Implement this through a registered rendering/
resolver contract and accepted response schema, not invented response fields.
Each immutable generation must map every unique handle to its exact source card,
revision/range/checksum; ambiguous, missing or wrong-generation handles fail closed.
The readable name is navigation, never source identity or semantic proof. Keep full
integrity fingerprints and original evidence. Compare valid-but-unrelated handles
and quoted passages as well as invented ones before accepting that design.

Only Home Assistant currently has static configuration adapters. Defensive server/
firewall configs, SQL and tomedo API payloads require their own versioned parsers,
schemas or non-mutating application checks. Never treat the YAML check as validation
of all other domains or run generated operational configurations against live
systems merely to validate them. Keep syntax, static
schema, semantic review and observed runtime/device checks separate.

Sempai additionally needs lossless proposal/whole-artifact validation through its
actual class-specific sink. Retain the draft before review; check supported fields,
immutable input identity, authorized prompt-edit boundaries, capabilities and the
entire component/association/workflow. Isolated bounded Linux pure-logic execution
checks are appropriate; unapproved candidates never dispatch Tools or mutate
devices. CPython success is not Monty compatibility or production Q1/Q2. Validate
summaries against payloads; prerequisites/warnings must survive in the actual
artifact, not exist only in a prose summary.

An optional contract-planning model call is untrusted intermediate data, not an
executable plan or approval. Each independent call/validation usage has its own
Recipe step. Version output schemas/grammars and pin their hashes and supported
parser behavior. The current optional planner schema contains `artifact_kind`,
`requirements` (aspect plus exact contiguous `contract_quote`) and `workflow`
(ordered knowledge/UUID references supplied by the original instruction). Mechanical
validation rejects empty/non-source quotes, non-Recipe workflows, unsupplied UUIDs
and extra executable branch fields. This does not establish classification, complete
coverage, correct order/bindings or behavior; the writer still reads the entire
original contract. Keep this planning schema distinct from persisted Recipe JSON.
Emit requested artifacts before `composition_summary`, and derive the summary from
the actual payload. Fixed-order grammars must fingerprint ordered serialization;
canonical key sorting or dictionary equality cannot establish decoder compatibility.
Protected-message/single-proposal/single-variant constraints apply
only when the trusted authoring contract requires them; offline benchmark settings
must not restrict general v3 Recipes. Unsupported fields/APIs are failures.

Bounded repairs consume only the retained failed draft and real validator evidence.
Record positive attempt limits and parent/response/feedback hashes with an unchanged
task contract. Never weaken checks to reach a score, replay effects, merge repairs
into first-pass accuracy or supply benchmark answers to first-pass requests. Keep
syntax, semantic behavior, citation support and serving failures separate. Quality
observations identify suite/revision, client protocol, decoding controls, runtime
fingerprint and first-pass/repaired results; absent measurements say Unmeasured.

## 11. Phased implementation and acceptance gates

### Phase A — Contracts and source inventory

Freeze profile IDs and source classifications; inventory current source packs,
existing generation receipts and actual provider adapters. Specify typed input/
result schemas, Recipe variants, one-UUID steps, Skill associations and failure
contracts. Resolve the trusted Tier-1 compiler admission and typed-input/state/
version prerequisites rather than copying obsolete Plan-2 seed examples. Add at
least ten positive intent examples per authored Recipe plus negative routing cases.

Acceptance: reviewed end-to-end workflow and exact missing primitives; no invented
UUIDs, unsupported store fields or false Tier-0/Q2/runtime claims.
Include the saved build specification, six seeded profile coverage matrices and custom-profile coverage contracts, artifact/
identity schemas, worker dependency/resource contract and a target capability matrix
for injection, token inspection, grammar, native/external cache and validation support.
Inventory per-profile compiler capabilities and differences, including supported
architectures, source adapters, projection/policy behavior, protocol schemas/order
and actual entry-point defaults. Bind that capability revision to accepted designs;
unknown options fail validation rather than disappearing during compilation.
Specify section 5.3's model/specification/runtime/design identities, immutable schemas,
profile/consumer selection rules, provider-switch impact and the confirmed design-job
contracts before implementing database or UI fields. Unknown capabilities stay explicit.
Specify section 8.1's used-versus-registered source inventory, registration/collection
contracts, per-type bounded checks, availability states and source-review eligibility.

### Gate A1 — Runtime prerequisites for production Recipe jobs

Track these as explicit dependencies with implementation references and passing
production-caller receipts, not as documentation-only completion:

- Consistent immutable catalogue selection, transitive revision/checksum pinning,
  exact Skill-association approval and retention for suspended tasks.
- Actual selected Tool implementations retained as immutable executable artifacts
  or handles, with checked content identity and adapter/ABI compatibility, following
  simplified-v3 Phase 0a item 5. Metadata checksums plus a mutable file/latest handler
  do not pass. Preserve artifacts through child execution, waits, retries and
  continuations; missing/incompatible implementations fail before effects. Drain
  or reconcile open work before an incompatible host/binary upgrade.
- Typed input/result binding without source substitution, task-owned intermediate
  state, isolated attempts and preserved child/wait/resume selections.
- The exact per-invocation failure/retry and durable dispatch/effect contract from
  Phase 0a items 9–10, including pre-effect persistence, durable counters, stable
  keys/arguments, eligible outcomes and unknown-completion reconciliation.
- Exact Recipe admission, no Tier-2 fallback, durable job/attempt cancellation and
  current global Tool-policy rechecks. Trusted Tier-1 admission is additionally
  required before offering a shell/Tier-1 variant; it does not block the accepted
  model-free baseline solely because an optional variant is unavailable.
- Accepted non-shell, zero-model-call generation/composite execution through the
  registered deterministic compiler adapter, so first build and invalidation recovery
  do not depend on the prefix being built. Shell variants retain Tier-1 requirements.
- The simplified-v3 Phase-3a Monty upgrade and global startup/readiness gate,
  including required caller compatibility, continuation, resource and recovery
  acceptance. Reuse its accepted implementation; this feature does not replace it.

Phase B's offline compiler/source-package work and Phase C's storage/migration
development may proceed while these dependencies are outstanding. Production
Recipe admission, startup-triggered generation and enabled UI Generate actions may
not. Do not substitute a private shell runner or the old per-turn VM to bypass
Gate A1. Label intermediate artifacts and surfaces as development/unavailable.

### Alignment with the simplified-v3 implementation sequence

| Prefix work | Required simplified-v3 contracts/acceptance |
| --- | --- |
| Phase A / Gate A1 | Phase 0 inventory, Phase 0a component/binding/artifact/retry contracts, applicable Phase 1–3 operator/policy paths, and Phase 3a upgrade/global lifecycle. |
| Phase B | Offline compiler work may proceed independently; installed seeding/upgrade follows shared integrity boot and Phase 6 migration contracts. |
| Phase C | Phase 0a durable execution/selection contracts and Phase 6 versioned storage, reference preservation and recovery. |
| Phase C1 | Sempai reviewer reference/packet/proposal contracts, Phase 0a authoring validation and human Q2, with consumer wiring accepted in C2. |
| Phase C2 | §10 DB-only provider lifecycle, live global policy and applicable Phase 7 production-caller acceptance. |
| Phase C3 | Accepted Gate A1/C/C2 paths plus DB model specifications/designs/bindings, immutable proposal/current-content generation contracts and Prefix-Maker bootstrap; authored executable changes follow Phase 0a Q1/human Q2 and registration. |
| Phase D | Phase 5 instance operator UI/API; reachable actions depend on accepted Gate A1/C/C2 paths, with C3 additionally required for design operations. Prefix knowledge profiles do not introduce security RuntimeProfiles or product editions. |
| Phase E | Phase 7 answer/cache acceptance and §5.1 performance evidence; quality and cache reuse remain separate measurements. |
| Production cutover | Phase 8 coordinated migration and component-cutover gate after the applicable prerequisite matrices pass. |

Reuse accepted upstream v3 work and receipts; do not reimplement its lifecycle,
provider catalogue or policy engine within the prefix feature. Independent offline
development need not wait for every unrelated v3 cleanup, but production paths must
pass all contracts applicable to their consumers.

### Phase B — Base compiler and release source package

Continue from the offline `base-promt-compiler.py` and shared `prefix_evidence.py`
adapter below; do not create another copied compiler. Complete installed catalogue
and release-document export, shared-engine parity and production worker qualification.
Port §4.2's v25 evidence-view and scoped-audit facilities deliberately; qualify each
profile adapter and keep Sempai consumer schemas separate. Implement the installed
source-package path, environment-independent worker inputs, model adapter capability
checks and raw-package publication rather than wrapping the shell example unchanged.
Build reproducible release source packages and seed their registration through
shared boot. Preserve original documents/code and complete structured records.
Package the dedicated Sempai compiler/procedure and full authoring/validation source
inventory in the same release process. Its standalone allowlisted checkout adapter
must become an installed immutable source-package adapter; installation cannot
depend on the developer checkout. Measure the complete rendered Sempai/base composite
against the selected model before promising readiness. If mandatory originals exceed
capacity, report the exact requirement and keep inputs intact; no automatic guide
truncation or implicit lower coverage is accepted.

Acceptance: every mandatory guide is complete; Recipe/code/schema/association data
survives export; no HA topic leakage; missing reference/coverage fails the build;
two offline builds of the same input produce identical source/evidence/template/
leading-token fingerprints; raw originals survive failed builds. Fresh install,
upgrade and reseed preserve integrity, operator edits and retained old versions.
Also require host/path-independent content identity (including removal of current
path-dependent model/source fields from canonical identity), reproducible whole-unit
selection and exact byte/hash agreement between source and original evidence.
For reviewed strict-JSON displays, require decoded-value and embedded-program
identity plus separately verified original/display/rendered hashes; do not demand
raw/display byte equality or normalize ambiguous JSON. YAML, Python and prose remain
literal. Require reviewed conflicting/versioned sources; tutorial mutation checks
and transitive validator fingerprints. Prove zero provider/network/process discovery
through the actual worker entry point. No optional synthesis mode is invoked.

### Phase B implementation slice — offline base compiler (2026-10-11)

Implemented an explicit reviewed-inventory snapshot adapter and immutable-package
base compilation in `scripts/prefix/base-promt-compiler.py`. It preserves original
bytes, full documents/code, source inclusion/exclusion reasons and complete section
ranges/hashes. Publication is staged, content-addressed and serialized per output;
no active pointer is changed. Identical packages compile independently of worker
paths. Strict duplicate-key parsing, checksum/root checks and mandatory-guide
coverage reject incomplete/tampered inputs before publication.

`prefix_evidence.py` now owns shared literal fences and real template token probes
for all four existing domain compilers; HA/Sempai also share section segmentation,
and HA/base share the existing Ornith/Qwen server injection adapter. tomedo PDF
segmentation and Defensive's existing suffix behavior stay in their adapters.
All affected generation identities include the shared module hash. Remaining
profile-specific quality/projection features have not been unified or requalified.

Capacity optimization renders each full source document once instead of wrapping
and duplicating provenance around each section. The full section/evidence index
and all original bytes remain in the artifact. With the actual cached Ornith
snapshot `489cb97981b8654bcfcf30ce1f94ed1b62e07b53`, the regression's seven complete
mandatory guides plus one actual component render to 119212 tokens, down from
140519. An explicit 8192-token workspace reserve gives 127404 of 131072 tokens.
This is the base adapter's documented minimum/default; larger workload reserves
are selectable and capacity failure never drops source material. Domain compilers'
existing reserve defaults are unchanged. Adding the full installed catalogue and
remaining release docs may exceed this budget and must be measured separately.
No claim of full-library fit, deployed model quality or KV/recurrent cache
qualification follows from this local tokenizer result.

The retained local development build at
`/private/tmp/brassclaw-base-prefix-sfn9bqdd/receipt.json` uses the real sample
component's repository path and shorter inventory reasons: 119195 rendered tokens,
118933 stable shared tokens across 18 distinct rendered probes, plus the 8192 reserve
(127387 total; 3685 remaining). Its generation is
`579e1125967d0686f87b9ca8971b6a35526eba8b4d6cf45970396b94aaa14d33`.
The receipt pins tokenizer/template and engine hashes; it remains a development
artifact with `activation_eligible=false`, not an installed runtime acceptance.

Acceptance: four offline base tests (including actual CLI/tokenizer, reproducible
builds and failed-capacity preservation); seven HA, ten tomedo and two Sempai
source tests; one cross-profile citation test. Defensive's two pure evidence tests
pass; its seven complete-model build tests were not run because the cached snapshot
lacks the required model config. This slice changes no Rust/runtime seed/DB/provider
or remote deployment. See [copy-paste usage](../../scripts/prefix/BASE-PROMPT-USAGE.txt).

Still open: complete current release inventory; full-library owner export beyond
the scoped ordinary-workflow projection accepted below; installation-owned source/compiler qualification; bounded registered
worker; runtime generation/composite Recipes; production named storage/jobs,
consumer resolution and UI. Development packages never establish approval or
activation (`activation_eligible=false`). The local manifest is an artifact contract,
not a new installed source/approval catalogue, queue or provider registry.

### Phase B follow-up — owner-selected catalogue source export (2026-10-11)

The new operator read projection is
`GET /api/webchat/v2/catalogue/{generation}/sources`, reached through the existing
`RecipeStore` / `RebornServicesApi` facade and installed catalogue owner. It shares
capture's immutable revision/usage approval checks; it holds the owner's activation
serialization and one read-only repeatable-read transaction. A requested stale
memory generation fails with 409; catalogue bytes/checksum, DB active pointer,
exact source revisions and retained usage approvals must agree. Exporting performs
no Tool dispatch, Q1/Q2 write, candidate activation or generation publication.

`brassclaw-catalogue-source-export/1` preserves exact catalogue bytes, component
revision envelopes/checksums and each complete retained selection, including
variant/input/export/approval pins. Components are deduplicated by exact revision;
conflicting selections fail rather than silently prefer one. The global root's
pinned source is included separately as retained evidence, without claiming that
retention qualifies an authored root or establishes usage approval. Scope is
explicitly `active-ordinary-workflows-and-retained-root`; legacy validated rows
outside that owner, separately managed review workflows and staged proposals are
not exported as approved active components. Full-library migration/qualification
and a complete release inventory remain prerequisites for the final base prefix.

The offline snapshot adapter accepts `--catalogue-export` alongside the reviewed
release inventory. It checks transfer/source hashes, identity, dependency and
selection coverage and preserves all exact originals. It renders component and
selection sources once; the full transfer envelope is retained as raw provenance,
avoiding duplicate component text in the model prefix. Imported JSON is not an
approval writer and all resulting development generations retain
`activation_eligible=false`. Capacity is checked against actual token rendering;
adding this graph never authorizes omitting any mandatory source to make it fit.
The 64 MiB export capacity is a supported technical transport bound, not a token
budget or row limit; exceeding it rejects the complete export.

Focused acceptance (2026-10-11): the exact frozen
`complete-cancellation-20261011-r2` binary passes the actual authenticated operator
component-deployment/source-export regression against native PostgreSQL and the
global worker (6.83 seconds). Its actual exported input then passes all five
offline compiler tests, including path-independent identity, exact original-byte
retention, transfer corruption rejection, immutable publication and the cached
Ornith tokenizer (10.95 seconds; no skipped cases). The seven mandatory documents
plus this owner-selected graph render to 119,212 tokens with an 8,192-token reserve.
This is below the selected 131,072-token capacity; the same sources reject a
32,000-token target without publishing or dropping sources. The live model's
`/v1/models` also reports 131,072 on 2026-10-11. The preserved receipt/export/
tokenizer hashes are `/private/tmp/brassclaw-prefix-export-accepted.json` and
`target/validation/prefix-export-artifacts-20261011-r2/accepted.json`.

This is focused native export and offline quarantined-compiler acceptance, not
production prefix activation or complete library coverage. The containing
snapshot's separate expanded cancellation test failed its fixture setup and is
being repaired; it is not counted as passing source-export evidence. The preceding
strict composition lint covers the same unchanged source/export implementation;
subsequent whole-matrix and production prefix-job acceptance remain required.
[Copy-paste instructions](../../scripts/prefix/BASE-PROMPT-USAGE.txt) describe the
existing operator token, pinned-generation export and subsequent offline build.
Runtime Recipe/worker/storage/UI implementation remains in the following phases.

### Phase C — Named generations, jobs and Recipe execution

Add forward migrations, registry/generation/job stores and compatibility import.
Version the existing assembly Recipe and add reusable snapshot/compiler/validation/
publication usages. Admit durable jobs through the runtime, preserving no-fallback,
completion fencing, cancellation and deduplication. Implement dependency invalidation.
Implement the model-free bootstrap path and composite Recipe/adapter/job contract
above, including parent exports, evidence identity preservation and recomputed token
geometry. Inventory existing build primitives before adding the missing worker adapter.
Production dispatch requires Gate A1; offline store checks alone do not pass it.

Acceptance: real PostgreSQL tests for six seeded profiles and registered custom profiles, coherent snapshots
during concurrent activation, full catalogue export above 1000 rows, invalid-source
rejection, job restart/resume/cancel, double-click deduplication, exact request
completion, no replay after successful publish, last-good preservation and rollback.
Failed Recipe/DB operations never become Tier-2 model work.
Include crash injection before dispatch-intent commit, after compiler/publication
effects but before result recording, and after completion before reply. Assert
stable invocation counts/keys/arguments, safe unknown-result reconciliation, no
effect after failed persistence, no confirmed-effect replay, and retained exact Tool
artifacts across child/wait/retry. Missing artifacts or incompatible ABI fail closed.
Test first installation with no prefix, no provider and pinned local artifacts;
regeneration after component invalidation; missing artifacts; and zero LLM calls
through the actual bootstrap caller. Test composite build/rebuild/cancel/restart,
parent races, evidence-ID conflicts, exact parent manifests, deterministic rendering,
coverage/capacity rejection and retained independently usable parent generations.
Test the normal one-click operation with missing/outdated parents and reused parents,
parent/target changes mid-operation, partial child failure and cancel/reconnect.
Reconcile published children without replay; an outdated completed artifact remains
historical. A source-generation-only operation without a provider completes honestly
without an activation-ready badge. A target-aware operation cannot report success
until its required composite validation completes.

### Phase C foundation — named profile registry (2026-10-12)

`V139__prefix_profile_registry.sql` introduces the instance-wide
`reborn_prefix_profiles` metadata registry and six shipped profile registrations.
Names, labels, descriptions, positive metadata revisions and compiler entry names
are separate from immutable model designs, generation artifacts and consumer
selections. Compiler entry names are descriptive registration data; no filename,
row or revision authorizes executing a compiler. Prefix-Maker has no registered
compiler filename until its actual adapter exists. Installation inserts preserve
existing metadata rather than overwriting operator edits.

The existing `InterceptorConfigService` / facade / operator prefix-list API and
Prefix settings tab now read this registry. Every seeded profile appears before
its first build, with a description and explicit readiness. Existing scope-bound
base bundle fields are compatibility projections only. Base generation still uses
the existing Recipe route and is labelled `legacy-unverified`; a prior bundle or
successful prewarm timestamp cannot produce a verified readiness badge. Other
profiles are `prerequisites-required` and Generate is disabled. A direct request
for a registered unsupported profile fails with 503 before a conversation, Tool
or provider call; an unknown name returns 404. Missing registry/store support fails
closed rather than inventing a frontend list.

This foundation does not implement named immutable generations, model-specific
accepted designs, durable generation jobs, sources controls, Redesign, custom
profile authoring, composite builds, production compiler qualification or new
provider selections. Keep those in the owning Phase C/C2/C3/D steps; do not treat
metadata registration as bootstrap approval or duplicate their stores in this
registry. The existing legacy base store and running selections remain intact.

Focused acceptance passed on 2026-10-12 using the frozen source and retained
worker/test pair in `target/validation/prefix-registry-artifacts-vg1dq56v/accepted.json`:
strict product/composition lint; the authenticated operator/native PostgreSQL
registry/export/deployment regression (7.83 seconds); all five actual-export/offline
compiler checks without skips (10.53 seconds); and three UI behavior checks.
The native regression verifies six seeded registrations, descriptions/revisions,
metadata edit preservation across migration reruns, 401 for missing authentication,
503 for registered unsupported generation and 404 for an unknown profile. It also
retains the existing coherent deployment/ordinary global-Monty acceptance.
UI checks verify disabled unsupported actions, honest legacy readiness and
fail-closed older API responses. Compilation against the actual cached tokenizer
still measures 119,212 rendered tokens plus an 8,192-token reserve; this establishes
local capacity for that tested package, not complete-library fit or cache readiness.

Validation for this chat uses the operator-requested
`/Users/ollama/brassclaw-target2` with its own lock and disk checks; it never cleans
or locks the other chat's target. The frozen source/receipt identifies exact tested
bytes; subsequent concurrent edits need their own applicable evidence. This slice
changes no inference deployment and does not enable production named-prefix jobs.

### Phase C storage — immutable compiler artifacts and source registry (2026-10-12)

`V142__immutable_prefix_generations.sql` adds content-addressed immutable generation
storage. It preserves legacy scope bundles as `legacy-unverified` records without
changing serving pointers or inventing source/tokenizer/cache evidence. Compiler
packages carry all original artifacts, source/evidence indexes, context, template
when measured and the exact compiler manifest. The base adapter exports
`brassclaw-prefix-generation-package/1` with canonical base64 artifacts; file export
uses serialized atomic publication. Import validates strict JSON, complete artifact
inventory, per-artifact hashes, the original compiler generation identity, mandatory
source coverage, byte-identical original/evidence text and declared token capacity.
Imported packages remain `quarantined`; integrity is not qualification or activation.
The current import adapter accepts the supported base compiler contract only.

The existing prefix facade now exposes operator-only generation list/detail/import:
`GET/POST /api/webchat/v2/prefixes/{name}/generations` and
`GET /api/webchat/v2/prefixes/{name}/generations/{generation}`. Exact lost-response
retry reads the immutable prior receipt despite later profile metadata changes;
a conflicting predecessor cannot overwrite or reinterpret the retained generation.
The database rejects update/delete/truncate. Generation history/inspection and base
package import use the existing Prefix tab. Client retries retain exact request
bytes/revision and suppress duplicate dispatch; import never changes active state.

Focused native acceptance is retained under
`target/validation/prefix-registry-artifacts-jkw9aoyc/accepted.json`: actual Python
compiler/exporter -> authenticated API -> PostgreSQL -> immutable reread; tampered
packages and conflicting revisions rejected; lost-response retry retained; legacy
store untouched; update/delete prevented. Route contracts and five offline compiler
checks also pass. Two generation-history UI checks cover exact retry/deduplication
and rejection of oversized files before reading/posting. This is storage acceptance,
not a production generation Recipe, source qualification or activation claim.

`V143__prefix_source_registrations.sql` adds immutable reference registrations and
separate append-only observation storage. The existing facade exposes
`GET/POST /api/webchat/v2/prefixes/{name}/sources`, with optional exact generation
selection for its archived source inventory. HTTP(S) references preserve meaningful
version/query/fragment identifiers; embedded credentials, recognized credential
query fields and unsupported schemes are rejected. Exact component references bind
UUID/version/checksum and verify immutable source bytes. Saving makes no fetch or
model call, grants nothing and leaves collection/review pending. Duplicate identical
registrations return the original; conflicting duplicate metadata fails.

Sources opens a profile-specific panel beside Generate, with generation selection,
separate used/registered inventories, archived originals, registration form and
text-labelled availability. No availability is manufactured: until the check Recipe
and trusted observation writer are qualified, checks remain explicitly unavailable.
Registration/API/native behavior passed in the frozen receipt
`target/validation/prefix-registry-artifacts-if58k53w/accepted.json`; that capture
records its lint limitation explicitly. Current strict lint, native/model/storage
and contained-Monty checks passed in
`target/validation/prefix-registry-artifacts-87h02exi/accepted.json` and its subsequent
storage-primitive slice `prefix-registry-artifacts-zr4r4mre/accepted.json`.
Do not count the source-check,
collection/review, invalidation or Redesign workflows complete from these records.
Generation jobs, target-aware compiler qualification, per-model designs, consumer
resolution and activation remain the subsequent required work.

### Phase C model/design foundation and shared rendering (2026-10-13)

`V145__prefix_model_design_records.sql` retains immutable model-configuration
snapshots against the existing provider namespace/ID/revision history, declarative
design proposals and separate future acceptance records. It does not seed a second
provider catalogue or accept a design. The facade projects active Kohai/Sempai targets
from its existing LLM configuration owner. An overridden model never inherits its
provider's default-model context as evidence. Capture checks the exact selected
provider revision and preserves architecture/weights/tokenizer/template/cache fields
as Unknown until real artifact/effective-runtime evidence exists.

Operator routes `GET /api/webchat/v2/prefix-model-designs`,
`POST /api/webchat/v2/prefix-model-specifications` and
`POST /api/webchat/v2/prefixes/{name}/design-proposals` use the existing authenticated
facade. Proposals accept only the implemented declarative adapter/view/coverage/
reserve options; no executable code, arbitrary compiler paths or success flags.
They remain unaccepted. Model Designs in Prefix settings lists active targets,
retained settings and proposal history; selection alone writes nothing. Retain
model settings is separate from provider-assisted Generate model design, which
remains disabled until its actual Recipe/Prefix-Maker/artifact prerequisites pass.

`prefix-maker-compiler.py` now reuses the base compiler's exact evidence/token
machinery and requires all seven BrassClaw guides plus
`scripts/prefix/prefix-maker-reference.md`. Its source-backed authoring reference
separates model/deployment identities, INT4 weights/attention/recurrent dtypes,
preload/host/invocation/Recipe contexts, decoder property order, complete evidence,
stable token construction, capacity, proposal/acceptance/activation and honest
cache qualification. It contains no benchmark answer key or deployment operation.
Current knowledge does not certify an unknown model architecture. Complete raw
guides and reference stay retained; overflow fails without deleting coverage.
`PREFIX-MAKER-USAGE.txt` provides exact Linux/macOS snapshot/compile/export commands.
Import accepts this profile's complete integrity-checked development package as
quarantined. `V147` fills the previously missing compiler metadata only for the
unmodified initial registry revision; it grants no execution authority.

`prefix_render.py` contains effect-free literal rendering shared by the offline
compiler and the packaged `prefix_reference_components` PythonCode draft. The
compiler manifest pins that renderer, shared evidence engine and profile adapter.
Rendering uses typed, independently verified ordered source data, no imports or
host effects, and invocation-local arrays. Its draft is not an activated component.
Actual contained-Monty rendering checks and native model/storage acceptance passed
in those frozen receipts. Eight focused WebUI checks cover model selection/capture,
Sources open/selection/save and generation-import retry/limits. Do not infer a
production generation workflow from utility evaluation or storage acceptance.
The closed installation bootstrap owner still needs to qualify actual
storage/tokenizer usages and the complete Recipe graph independently.

Implementation direction remains Recipe-first: retain snapshot read, pure rendering,
tokenization, immutable publication and optional model evaluation as distinct reusable
usages. Rust may supply fixed immutable-artifact/store and tokenizer primitives.
If retaining an external Python runtime, pin its actual interpreter/dependency closure;
a mutable host Python pathname plus script checksums is not a retained implementation.
Do not widen the reply-only bootstrap owner to accept generic seed labels: it verifies
exact reply definitions, adapter and behavioral cases. Qualify the prefix usages through
a closed subject verifier and real independent behavior, then integrate its verified
approval into both catalogue selection and readback. Extract shared internal admission
from existing turn admission/waiting, retain durable correlation/scope, and link prefix
job metadata to those runs. No background wrapper around the blocking sender, new queue,
new workflow loop or special unapproved bootstrap executor closes Gate A1.

`literal-reference/v2-deduplicated` is an explicit supported development design
option. It merges only byte-identical complete units, retains ordered original
source/range reconstruction and renders unique neighboring qualifications intact.
It never summarizes or drops required information. `render_map.json` is independently
checked against actual original bytes/ranges at immutable package import, including
CRLF and Unicode line separators. Shared-unit IDs are rendering identities, not
document citation IDs. Default `literal-reference/v1` remains unchanged; adopting
v2 requires explicit design selection/acceptance rather than a hidden Generate change.
The frozen Prefix-Maker geometry comparison is retained at
`target/validation/prefix-maker-geometry-cdi_07q2/comparison.json`: v1 measured
120729 rendered tokens, v2 117628; both had 18 rendered probes and an 8192 reserve
within the declared 131072 context. This comparison predates the added citation-ID
clarification sentence; rerun for exact successor counts. It certifies local geometry
and original reconstruction, not deployed model quality or external cache correctness.

`builtin.prefix_generation` is now a concrete registered immutable-storage primitive
with Read and Retain operations over the existing `PgPrefixGenerations` owner. It
neither compiles nor chooses/activates a prefix. Read returns exact bytes/qualification;
Retain returns the immutable quarantined receipt. It rejects extra/activation inputs
before backend effects, bounds transport and fails without a wired backend instead
of using legacy mutable storage. Canonical `prefix_generation_components` drafts
define separate reusable usages and Recipes. Retain's revision parsing is a separate
pure PythonCode step; binding immediately precedes its one Tool invocation. The drafts
declare typed contracts, preload interfaces, stop-only failure semantics and ten
command variants. Actual linked registration/schema/image verification, PostgreSQL
backend/reread/conflict, view-v2 import, contracts and negative primitive cases passed
in `prefix-registry-artifacts-zr4r4mre/accepted.json`. This is primitive/draft evidence,
not association approval or actual qualified Recipe admission. Complete the closed
bootstrap subject/behavioral producer and its catalogue selection/readback integration
before admitting production generation jobs.

The closed qualification input must be genuinely source-backed. The installation
adapter embeds the seven actual packaged guides, archives their complete originals
and evidence ranges, and executes the shared effect-free renderer through the
captured Monty utility worker under supplied preparation limits. Its manifest says
`installation-qualification-baseline`, `documentation-only`, token geometry
unmeasured, quality unmeasured and cache unverified. It retains the actual assembly
and renderer source identities and does **not** claim the offline compiler ran.
This baseline qualifies storage behavior only; it cannot stand in for a complete
approved component-library source export or a production base prefix. Never add a
hidden fixture table, synthetic guide text or invented compiler hashes to satisfy
an approval check.

Candidate retention must compare the exact packaged Tool, Skill, PythonCode,
ToolSkill and canonical Recipe documents after immutable storage, compile the typed
`selected` variant and inspect its exports using the captured worker. Initialize
only missing native Tool settings; preserve an existing operator block. Candidate
retention itself neither approves nor changes active routing. Shared contained
admission/transport/settlement mechanics may be reused from reply qualification,
while each usage retains its separate behavior oracle and closed producer identity.

Retain qualification executes the pure decimal preparation at `0:1` **and** its
one actual storage dispatch at `0:3`; Read dispatches at `0:2`. Verify complete
SQL-reread bytes, immutable duplicate receipt/timestamp, malformed package rejection,
predecessor conflict, live-policy denial and zero dispatches on invalid decimal
preparation. Read covers exact bytes, missing/malformed identity and live-policy
denial. Check the actual invocation journal, one initial attempt, settlement and
unchanged active catalogue/legacy serving selection. Development checks cannot
substitute for committed producer evidence. Persist a subject-specific Q1 intent
before behavioral effects, retain actual observations and exact association approval,
and independently verify the durable rows on readback; incomplete intents require
reconciliation, never automatic case replay.

The retained Tool-parameter parser must preserve genuine omission separately from
Recipe consumer defaults. A Tool can expose optional operation-specific parameters
without supplying null or a synthetic default. Keep the ordinary task/step input
constructor's required-default rule; use the distinct Tool-parameter constructor
only when preparing the registered Tool contract. Supplied null/invalid/undeclared
arguments still fail, declared defaults are validated and materialized only for
missing values, and cross-parameter inequalities fail if a supplied operand's
required comparison target is absent. Do not work around this gap by assigning
multiple stable Tool identities or incompatible per-usage Tool revisions to the
same native primitive.

The current catalogue's combination-coverage check does not cover standalone pure
Recipe components through a Skill association. Add narrowly scoped canonical-workflow
coverage for Retain's exact formatter only after the complete selected packaged
Recipe has matched its committed qualification subject. Reusing the storage Skill
elsewhere does not qualify another Recipe's pure code. Do not add a generic pure-code
exemption or accept arbitrary records labeled `system_seed`.

Installation qualification groundwork is implemented and checked in
`target/validation/prefix-registry-artifacts-anxaw5bg/accepted.json` (immutable source
`prefix-registry-source-rqhphieu`, captured matching worker and native test binaries).
The installation retains both exact Read/Retain candidates and the actual linked
native Tool definition without activating them; existing Tool blocks remain intact.
Shared admission/transport/settlement and immutable proof-storage mechanics are reused
with the reply qualifier, whose ordinary matched/No-Match regression also passed.
The closed installation-owned generation proof producer now commits both exact
usage approvals before runtime readiness. It records subject-specific Q1 intent
before effects, executes Retain's parser/storage and Read through real contained
Monty and PostgreSQL, atomically retains behavior/association records and verifies
the complete readback. Existing committed proof is reused without another dispatch;
partial qualification requires reconciliation rather than automatic replay. Coverage
of Retain's formatter is restricted to its exact canonical workflow. Candidate
retention and usage qualification do not activate a generation/compiler job.

The stronger producer passed in
`target/validation/prefix-registry-artifacts-dtr9ngal/accepted.json` (source
`prefix-registry-source-aibwq6cr`, matching captured worker/test binaries, target2).
Expected rejections must be Python failures from the same Ready worker/root and the
expected Start or ResumeHost command, not a timeout, crash or unrelated transport
failure. Readback counts all run invocations and rejects another Recipe's dispatch.
The malformed submitted package's SHA must be absent before and after the cases
and on committed readback. The actual journal, exact arguments/answers, initial
attempt, full baseline bytes and immutable duplicate receipt/timestamp are checked.
Serving selection remains unchanged. Canonical preparation/coverage, startup-owned profile registration, complete legacy
archive and domain procedure checks subsequently passed in
`target/validation/prefix-registry-artifacts-iga3n4b4/accepted.json` (source
`prefix-registry-source-0jef7hzv`). The exact Read/Retain Recipes prepare through the
real catalogue owner; a different Recipe cannot borrow the formatter's coverage.
The native Tool now uses its ordinary registered catalogue UUID throughout candidate
retention, approvals and live-policy lookup. Registration metadata remains pending;
its label does not supply approval and creates no installed human-Q2 requirement.
Do not infer full compiler-job admission from this storage-usage acceptance.

The general retained Tool parser now has a distinct
`InputContract::tool_parameters_from_value` path. Omitted optional parameters stay
absent; no null/default is fabricated. Ordinary Recipe consumer-input parsing still
requires its declared default. Contract regressions, registered capability inventory,
strict affected-package lint, real native workflows and offline compiler checks pass.
The pure renderer searches literal spans/control markers instead of iterating every
ordinary character. The preserved renderer independently establishes identical bytes
on all seven guides and edge cases; `render-equivalence.json` records the source and
output hashes. Full-guide contained Monty now completes in approximately 5.8 seconds,
where the prior implementation exhausted its 30-second deadline. This is rendering
performance evidence, not a model/cache-quality claim. Validation workers derive a
finite frame from the complete encoded baseline, align their value bound to that frame
and account for both frame buffers and adapter headroom within their existing physical
ceiling; no instance memory or deployment setting is changed.

Offline base and Prefix-Maker packages now retain the complete compiler entry,
shared evidence helper, renderer and profile-adapter bytes alongside their hashes.
They reject a procedure change during compilation before publication. Their explicit
`unqualified-development-process` attestation records that byte retention is not
trusted execution or production worker qualification. The same accepted snapshot
checks actual source bytes, immutable imports, real catalogue export and local
cached-tokenizer geometry; it does not establish the serving tokenizer's identity.

Legacy import must preserve all current compatibility-row fields, not merely bundle
text/scope. Forward migration V152 archives `to_jsonb(original_row)` with a v2 legacy
contract while retaining V142's immutable historical partial records. It neither
invents previously omitted historical fields nor changes any serving pointer,
operator profile label/revision or qualification. Its existing-configuration
migration fixture and repeated-import behavior passed in the same accepted snapshot,
including every original field, operator metadata and unchanged serving generation.
Startup explicitly retains the six first-party profile definitions before qualification,
independently of WebUI construction, and preserves existing labels, compiler entries
and revisions. Repeated retention does not create generation/readiness evidence.

The new `prefix_build_worker.py` development interface consumes a bounded
`brassclaw-prefix-build-request/1` byte frame: exact source manifest/artifacts,
fixed base/Prefix-Maker profile, supported declarative view/reserve and an optional
local tokenizer target. It accepts no compiler pathname, provider, collection,
deployment or activation instructions. Sources are checked and materialized only in
an isolated temporary directory; complete output is returned as an immutable package,
never a partially published generation. The actual protocol worker bytes/checksum are
retained in its compiler provenance. Target tokenization currently permits only the
registered `hf-qwen2-fast/1` adapter, local artifact names and no remote mappings or
external file references. Other adapters require implementation/qualification.

Independent CLI reproduction and network-denied source-only checks pass locally for
both supported profiles. The actual downloaded model config/tokenizer originals are
retained at `target/validation/prefix-model-source-cacik5n7/provenance.json`; they came
from a read-only copy of the registered model host snapshot, not invented metadata.
Nine domain compiler regressions pass using those files, including a roughly 99k-token
stable prefix and preservation after a failed rebuild. The shared `prefix_procedure.py`
helper now preserves entry/helper bytes and source-change rejection for Defensive,
Home Assistant, tomedo and Sempai. Their legacy artifact formats and worker adapters
still need full shared-engine migration; byte preservation does not establish parity.

The byte worker is a development interface, not a registered qualified runtime Tool:
complete procedure/dependency/interpreter retention, resource/cancellation enforcement,
actual native dispatch, durable job inputs/correlation and closed behavioral approval
remain required before the Generate action may call it. The generic storage importer
checks an optional worker checksum against retained bytes and rejects an undeclared
worker artifact. Production model input/specification ownership must preserve tokenizer
and configuration originals independently of every prefix package; do not duplicate
large tokenizer files per generation or substitute a stored hash for retained bytes.

Generation jobs must reuse the existing host-selected `TaskEntry::InternalRecipe`
transport and global Monty service. The worker wire protocol is now 15; capture and
use the worker from the same build. Public user text cannot select this private
entry. Implement the exact internal prefix-job resolver/owner and durable job
correlation; do not add a second queue, VM or public-command fallback. Check the
actual durable invocation JSON limits as well as the primitive's 64 MiB transport
ceiling before approving large package Read/Retain execution; technical failures
must preserve evidence rather than silently clip artifacts.

Runtime admission is factored into the existing `admit_user_message_internal` boundary;
the blocking caller retains its send lock and completion/history hooks. Matched and
No-Match global-Monty regression passes with retained VM identity. Durable internal
scope/correlation retention, asynchronous admission API and prefix job references still
need implementation; the existing ephemeral one-shot scope map is not recovery authority.

### Phase C continuation — large values, model originals and Recipe compiler (2026-10-15)

The accepted `target/validation/prefix-registry-artifacts-zjnchvho/accepted.json`
freeze exercises a real approximately 10 MiB source package through typed Monty,
registered generation dispatch, PostgreSQL invocation persistence and immutable
reread. V154 widens the invocation data columns to their finite 64 MiB transport
capacity. V155, supplied by the shared resource owner, retains each invocation's
admitted data limits; preserve that contract rather than replacing it with a new
hardcoded limit. Immutable schema/revision document limits remain separate. These
checks establish large-data custody, not complete-catalogue quality or activation.

V157 adds content-addressed model artifact sets and exact specification bindings.
The operator API imports original model configuration/tokenizer/template files;
the server derives checksums and authenticated provenance, deduplicates identical
sets, and exposes immutable originals separately from the small metadata catalogue.
The model-design panel accepts fixed artifact filenames for the selected exact
specification and coalesces duplicate submissions. Stored originals do not identify
the running server or qualify a model design. Native database/route acceptance passes in
`target/validation/prefix-registry-artifacts-icy572v0/accepted.json`, together with
four focused browser-module tests. This freeze preserves a previously compiling
merged dependency snapshot plus the upload-ceiling changes; it does not certify
newer concurrently edited resource code. The accepted prefix implementation bytes
still match the workspace at that receipt. The real
40 MiB import exposed an outer-ingress mismatch: the route declared 64 MiB while
the default outer cap was 14 MiB. The default now matches 64 MiB; smaller route
limits and explicit operator-configured smaller ceilings remain enforced, with
focused ingress checks. Do not solve this by disabling body limits. Preserve
V151/V153 embedding changes and V155/V156 resource/Tool-identity changes owned by
parallel work; migration numbers must remain unique.

Source-only compilation is now being expressed as reusable class-22 components,
not as a specialized Rust compiler workflow. `prefix_compiler_components.rs`
packages an effect-free section library, the existing literal renderer, a compiler
library and five small typed stages. `source_compile_recipe_draft` sequences:

1. Validate the exact positive profile revision using the existing formatter.
2. Prepare the complete pinned source snapshot in PythonCode.
3. Invoke the reusable JSON-Tool UTF-8 SHA-256 usage, then prepare references.
4. Hash evidence identities, then render the literal or deduplicated reference.
5. Hash the complete artifacts, then construct their identity.
6. Hash that identity, then finalize the immutable package.
7. Invoke the existing immutable Retain usage once.
8. Format that actual quarantined result into an internal-task receipt; the job
   owner must verify its correlated durable row and dispatch/effect settlement.

Every independent Tool dispatch has its own immediate ToolSkill/PythonCode pair.
The `sha256_utf8` JSON primitive hashes one ordered finite batch without filesystem,
network or workflow decisions. It preserves UTF-8, newlines, ordering and duplicate
values. The canonical digest Skill/Recipe is reusable outside prefix compilation.
The five compiler stages perform JSON/Base64/formatting in Monty and never call a
Tool. `prefix_units.py` is shared with the offline compiler; all affected compiler
procedures retain its original bytes and checksum. CPython oracle checks cover
both profiles and both reference views; actual Monty and retained-graph checks
remain required before qualifying these drafts.

This first Recipe produces **source-only, unmeasured, quarantined** base/Prefix-Maker
packages. It does not implement source collection, target tokenization, accepted
model designs, provider selection, jobs, composites or activation. More than 8192
strings in one digest batch currently fails the technical operation bound; implement
qualified Recipe batching before claiming arbitrarily large complete-library builds.
Do not omit source rows/units to fit that operation. Complete procedures in the input
snapshot must come from the selected retained code graph, not caller-supplied claims
about which code ran. The generation importer checks retained procedure links but
cannot turn arbitrary caller bytes into trusted execution provenance.

See [Recipe-owned source compiler](../reborn/prefix-source-compiler.md) for the
component/input/receipt contract. Development acceptance now covers the complete
preloaded compiler graph, exact CPython/SHA oracle agreement, actual registered
hash/storage calls, duplicate publication, missing mandatory source rejection and
live policy blocking. These checks do not manufacture bootstrap approval or enable
admission. The preload interface forbids Python default arguments: the shared unit
helper takes both arguments explicitly, while its typed public contract supplies the
`.md` default. Recursive result fields must declare requiredness, and producer
constraints must guarantee their consumer's bounds before the first effect.

Model artifact originals also expose a separate capacity hazard: the imported
`tokenizer.json` configures 1024-token backend truncation. A direct low-level encoder
inherits that setting; it is not the model's context limit. The existing HF probe
path validates the complete decoded leading reference, so a truncated probe cannot
qualify it. Any new tokenizer adapter must explicitly disable truncation/padding,
retain those effective settings and check the entire rendered request, including
its tail and task/output reserve. Keep model architecture capacity, configured
provider context, tokenizer defaults and runtime cache layout as distinct evidence.
Untruncated raw guide-only views measured about 115k–120k tokens in a development
probe; that is not full catalogue coverage or a serving/cache qualification.

The same accepted freeze passes the complete source compiler through preloaded
Monty, registered native dispatch and real PostgreSQL; its output equals the
independent CPython/SHA oracle exactly. It also passes large-value persistence,
existing catalogue activation/export, model artifact import/readback/deduplication,
legacy/profile/Tool identity preservation, tokenizer-aware offline workers and all
four domain regression suites. At that freeze, Q1/approval and durable Generate admission remained
open; accepted source-only qualification is recorded below. The neutral
`SourceCompilerIds`/`source_compiler_drafts` factory supplies one component-definition
source for installation and behavioral checks, retaining the existing canonical
Retain usage rather than copying it.

Installation candidate retention, canonical JSON-list command parsing, exact
retained subgraph selection and preservation of an existing JSON Tool identity/
operator block pass in `target/validation/prefix-registry-artifacts-_knynqqf/accepted.json`.
The factory now supplies both source and digest candidates without claiming approval
or activation. Source tasks pin their own complete dependency closure; unrelated
canonical Recipes are not carried into the task manifest. Captured command strings
are parsed by explicit pure PythonCode into typed lists, never implicit coercion or
source substitution. JSON usages share a typed JSON-text transport while the native
primitive preserves structured-call compatibility.

The archive correction passes before qualification in
`target/validation/prefix-registry-artifacts-e6fzt2b9/accepted.json`: older output
`sources.json` retained input-package object names although generation originals
live under `raw/<hash>`. New `brassclaw-prefix-generation-sources/1` inventories
name actual archive files and pin `input_artifact`: the exact canonical Monty source
snapshot or exact offline input-manifest bytes. All builds retain their actual
profile frame/design, including literal-v1 base builds. Validate original/output
metadata, evidence identities/ranges/counts and reject re-signed contradictory
links; preserve legacy input-namespace manifests without rewriting old generations.
An input manifest changed during compilation must leave the last good generation
and publication state intact. This is evidence integrity, not model/cache quality.

The internal-task adapter now has an explicit host-owned selection contract and
receipt verification hook. The bounded development observer runs the actual
packaged global Python root and its normal Recipe flow; Rust no longer needs to
repeat the compiler's step order or input wiring in that acceptance test. The
observer supplies no approval and is not a second production execution service.
Its negative checks must identify the original contained Python rejection,
expected invocation/ResumeHost origin (including cooperative ResumeControl for
long pure execution), unchanged Ready root and actual Tool answer;
a crash, timeout or framing failure is not the expected rejection. Successful
completion additionally checks immutable PostgreSQL package bytes and the native
Retain answer against the returned receipt. Shutdown/reaping precedes closing the
private validation claim, including error paths. Ordinary chat completion still
requires its verified reply, while internal completion requires its verified
receipt. The exact installed source graph and this internal Python execution path pass in
`target/validation/prefix-registry-artifacts-bq4q3hc0/accepted.json`. That receipt
is explicitly scoped to the native integration test, not the whole validation
suite or later source edits. It covers exact package/oracle agreement, duplicate
retention, missing-guide rejection, live policy blocking and run-wide SQL records
matching the observed argument/answer fingerprints, selection and attempt count.
An intended pure rejection may follow a cooperative compute-slice yield; requiring
only the initial Start command would incorrectly reject that valid observation.
These checks do not enable Generate.

Validation builds for this work use only `/Users/ollama/brassclaw-target2`.
Immutable source captures preserve concurrent changes without changing the other
chat's target or reserving shared build windows. A preserved dependency freeze
cannot establish compatibility with newer shared interfaces: capture the current
merged code when those interfaces change and report its actual check results.
The compiler oracle's transport capacity must use the largest individual exchange,
not the concatenation of all independently saved oracle stages.

The same complete validation freeze passes the independent closed-fixture oracle
for both profiles and render formats, including exact shared-unit origin maps,
complete originals/procedures, historical-source exclusion and rejection of a
coherently re-signed different frame. Its small synthetic originals qualify no
domain knowledge or model behavior. The native fixture-matrix producer/readback
checks are now being added as a separate development slice: four profile/view
successes, duplicate retention, missing guide, blocked Tool and invalid revision.
They must compare all invocation rows for each run with the actual observations,
recompute UTF-8 digests independently, reread immutable original packages and
receipts, and reject altered proof records in a coherent database transaction.
The evidence reader must derive the expected fixture from the closed profile/view
inventory, require unique run/root/thread identities, match the exact retained
selection and completed/failed steps, and check original-UTF-8 digest results against
independent SHA-256. Rejected pure work may legitimately fail after a cooperative
ResumeControl; rejected Tool work must retain its actual authorization answer and
ResumeHost origin. Include a complete fenced YAML original with CRLF endings and
an embedded model control token: archive bytes stay exact, and only the model-facing
literal view escapes that token using an outer fence longer than the inner fence.
These tests create neither installation approval nor catalogue activation. The eight-case native producer and coherent evidence reader pass in the scoped
`target/validation/prefix-registry-artifacts-yrg3_na1/accepted.json` freeze. The
installation writer acceptance below pins the complete packaged
graph, native implementations, captured worker, observer/validator/oracle sources
and existing canonical Retain approval; commit its exact Q1 intent before effects;
and atomically store verified behavior with its hash-usage approval. Private
qualified compiler selections are separate from routable chat Recipes: their
result is an internal receipt, so qualification must not advertise a normal chat
route. Test proof reuse with zero new dispatch, rollback-only incomplete-intent
rejection, exact association pinning and denial of public-route promotion.

Namespace synthetic fixture artifacts by the closed qualification identity.
A new binary/native qualification can otherwise collide with an unchanged
fixture package retained under an older profile revision. The same qualification
still reuses exact fixture bytes; older records remain immutable. Scope belongs
to validation metadata, never the production prefix design or model-facing text.
The scoped observation record uses `source-compiler-behavior-observations/2` and
its reader rejects a different scope; old proof bytes remain with their original
producer/subject. Treat the closed writer as accepted only after its native checks pass; incomplete
qualification intents require reconciliation without automatic effect replay.

The closed compiler writer, scoped/YAML fixtures and private approved selection
pass in `target/validation/prefix-registry-artifacts-p6m1aifn/accepted.json`.
This is the accepted source-only installation slice: exact graph/native/worker
integrity, committed intent before effects, actual eight-case behavior, coherent
proof readback, existing Retain approval reuse, atomic hash-usage approval,
zero-dispatch proof reuse and incomplete-intent rejection without modifying
immutable completed evidence. Normal matched/No-Match execution, archive integrity,
model artifact APIs, large values and all domain compiler regressions also pass in
that freeze. It does not implement production Generate jobs, accepted model designs,
provider resolution, composites, activation or cache qualification.

Next, reuse the global `InternalRecipe` entry and existing durable turn queue,
attempt retention and settlement owners for asynchronous Generate admission.
The initial normal-driver transport constructed only Chat entries, rejected internal
completion receipts, and verified result references only through thread messages.
The neutral admission/completion extension passes the complete
`target/validation/prefix-registry-artifacts-vxkw6s17/accepted.json` freeze;
the two internal-reference/outcome contract tests also passed against that exact
captured test binary. Prepare entry data and ports together, admit only host-selected
internal work, represent a verified receipt as a run-bound `result:monty.` reference
and reread the exact privately settled admission/scope. Match the persisted source
binding before admitting its trusted-internal marker. Ordinary chat retains transcript
verification; internal completion cannot carry a fabricated assistant reply.

Immutable job definitions pass the scoped native
`target/validation/prefix-registry-artifacts-dxv273th/accepted.json` freeze. V167 links
one definition to the existing turn run UUID and retains request, source inputs,
profile revision, complete compiler selection, actor and data scopes. The first capture
survives same-request retries; conflicting requests and definition mutations fail.
That receipt proves metadata/idempotency only, with no new admissions or Tool calls.
It does not qualify a Generate action.

The source-only snapshot submission/resolver passes the scoped native and
standalone-consumer freeze in
`target/validation/prefix-registry-artifacts-c5z_1d40/accepted.json`. Reuse ordinary durable ingress, the existing global service, retained
attempts, native invocation journal and result settlement. Reconstruct transport
scope from the retained definition after reconnect rather than relying on a one-shot
in-memory map. Verify the exact closed compiler approval and program bytes before
selection; never replace a retained selection with a newer compiler. Match archived
source data and compiler-procedure bytes to the actual pinned execution. Internal
builds do not advertise a provider/MCP exchange or send model requests. The native test verifies real
ResultOnly completion in the same global VM, no synthetic assistant reply, correlated
immutable output, duplicate submission without additional dispatch, conflicting
profile rejection and rejection of misidentified compiler procedures before retaining
a definition. Strict affected-library linting, the two internal-result contracts and
three actual standalone driver/cancellation/preparation regressions pass in that
freeze. This low-level snapshot boundary does not collect authoritative
sources or implement the normal WebUI Generate action. An incompatible restart still
requires retained-artifact recovery or explicit reconciliation, never a latest lookup.

Do not publish this compiler as a chat Recipe, invent a reply, copy the review
executor or create another VM/job loop. The existing reserved internal source binding
marks host-created prefix turns and prevents Tier-2 fallback. Preserve that host-only
proof; never infer it from a user's text.
The database-backed core-source capture passes the scoped native/strict
standalone freeze in
`target/validation/prefix-registry-artifacts-lbw2_5kw/accepted.json`. It reuses
class-17 revisions in `PgComponentRevisionStore` for the seven component guides,
Prefix-Maker reference and declarative source-only framing. The installation owner
compares the complete captured document graph to its actual packaged bytes. An
editable `reborn_docus` row is only a compatibility projection; reseeding never
overwrites it, and its source/status labels cannot establish packaged authority.
New packaged revisions and unapproved operator revisions remain retained separately.
The core collector reads exact package revisions plus the existing coherent catalogue
export, preserving both complete provenance envelopes. It does not scan a developer
checkout, look up newest document heads during execution, fetch network references,
qualify a target model or pretend to cover private workflows/all documentation.
`core-reference-designs.json` supplies a fixed literal-v1 source-only frame; this is
not an accepted model-specific design. Normal complete-library collection and the
remaining profile adapters are still prerequisites for the full Generate surface. The native test
checks exact originals, idempotent capture, preservation of editable Docu projections
and newer unapproved revision heads, rejection of forged packaged bytes, and real
core compilation through the existing global VM with no assistant reply or repeated
effects on reconnect. Its captured input was 1,370,747 bytes and observed completion
about 13 seconds; this is one local source-only observation, not model/cache or
performance qualification. Use a bounded large-build test observer, not the default
ten-second chat fixture wait, while keeping real task/resource limits unchanged.
The actual routing catalogue is included alongside selections so disabled Recipe
metadata is not hidden by a provenance-only transfer envelope.

Read-only job inspection passes the scoped native/strict-standalone and route
contract freeze in
`target/validation/prefix-registry-artifacts-oboa7t42/accepted.json`. It uses the existing
`InterceptorConfigService`, facade and authenticated v2 ingress. It projects the
immutable request/compiler/input hashes plus actual turn/admission/operation state;
status polling must not reload large source/compiler blobs, drive a task, fabricate
completion or repeat effects. Verify run/source-binding/scope/actor correlation and
only expose a completed result after its real admission settlement and turn completion.
A definition retained before a lost admission remains distinct from a queued run.
Record counts are observations, not reconciliation evidence or retry authority. The next metadata revision supplies the existing opaque thread/run references
for shared event/cancellation controls; do not introduce a parallel cancellation
workflow or infer that unresolved native work has been reconciled.



The normal Generate action remains gated on the complete job/source/target
contract. First-party bootstrap evidence stays distinct from authored human Q2.

### Phase C continuation — contained model encoding (2026-10-17)

The model-specific measurement prerequisite now has an offline encoding engine
(`brassclaw_model_encoding`) behind the existing retained `monty_worker` utility
boundary, not another global VM, process supervisor or Python environment.
Utility protocol 4 adds one `EncodeModel` operation; exact worker/image changes
must invalidate incompatible qualification rather than accepting an older
protocol. Assets and contexts are typed data; no tokenizer remote code, template
filesystem loader, source selection or provider calls run in this operation.

Accepted frozen offline evidence:
`target/validation/prefix-registry-artifacts-dvpfuk_0/accepted.json`.
It covers three focused engine tests, an explicitly executed retained-Ornith
Hugging Face oracle and an actual contained-worker test. The oracle uses offline
`AutoTokenizer.from_pretrained(..., trust_remote_code=False)` and the complete
retained six-file model set. It compares full rendered-byte and token-ID hashes,
counts and common-prefix hashes for long system text, tool schemas/arguments,
Unicode, reasoning/history and image placeholders, plus a separate 2,048-value
finite-float corpus. This is finite target-specific evidence, not universal
chat-template equivalence, a server/cache measurement or model answer-quality
acceptance. Python is an independent development oracle, not a runtime dependency.

Keep JSON object order all the way through nested tools/arguments. The renderer
must use Python/Hugging Face JSON spacing and escaping, rather than HTML-safe
Jinja JSON. Preserve exact numeric float parsing/printing; the initial larger
oracle exposed different token IDs without JSON float-roundtrip parsing. Saved
tokenizer truncation/padding are explicitly disabled; adding special tokens after
a completed chat template is disabled too. The long-input regression proves that
the model tokenizer's saved 1,024-token truncation does not silently shorten the
measurement. Template wall-clock functions fail closed.

The contained fixture explicitly requests a finite 1 GiB utility-worker memory
ceiling and 120-second response deadline. These are test preparation settings,
not automatic production overrides. Production must capture acknowledged limits
and visibly report insufficient capacity. The shared VM heap and task compute
account remain separate. Timeout/reap failure is never successful measurement or
permission to activate a prefix.

The `builtin.model_encoding` native adapter and canonical single-usage
Skill/PythonCode/ToolSkill/Recipe drafts are implementation work in this slice.
They resolve an exact specification/artifact-set binding from the existing model
store and share the already retained preparation worker/settings owner used by
source inspection. No second model catalogue, pool, queue or cache store is
introduced. Native registration, real PostgreSQL execution, current policy,
exact usage inspection and consumer regressions require their own acceptance
receipt before this paragraph can be promoted to a completed production claim.
These drafts are not approved/activated merely because the engine oracle passes.

Next, bind this usage into the generation Recipe's accepted per-model design,
freeze full provider render contexts/probes, compare required stable-prefix IDs
and context/output headroom in reusable PythonCode, and retain target evidence.
Keep full model/config/runtime fingerprints and actual attention/recurrent-state
cache qualification separate. A tokenizer measurement does not establish KV
capacity, LMCache persistence, salt isolation, cache hits or answer accuracy.
Normal Generate remains gated on those remaining source/design/job contracts.

The first native adapter measures the exact chat template in the retained model
artifact set. That is not evidence of the serving server's effective injection
template. Pin original model-template bytes and effective target/injection-adapter
bytes separately. In particular, an ordinary system message can follow a dynamic
tool-schema preamble: a long reference inside that message does not create a
common leading cache prefix across different tool sets. The accepted adapter and
probe workflow must cover the actual tool/no-tool, role/content, history and
thinking variants, measure complete rendered requests (never concatenate separately
encoded prefix/suffix tokens), and reject an incompatible stable-prefix claim.

Retained-task prefix lookup is also part of this boundary. A missing, expired or
poisoned run capture must return a typed prompt-assembly failure rather than read
the current active catalogue. Scope/correlation must match the captured task.
The compatibility unscoped lookup is not the retained-task resolver. Fail-closed
lookup is a prerequisite; it does not establish durable prefix/model selection
restoration after restart. Retain generation, model/target/design and complete
render-adapter identities through the existing continuation contract before
claiming that restart/resume or the new prepared-message model usage is qualified.

#### Native encoding integration and observed capacity

Frozen receipt `target/validation/prefix-registry-artifacts-0a0vlb9z/accepted.json`
now covers the three focused engine tests, offline full AutoTokenizer oracle,
actual contained-worker operation, affected-library strict lint, standalone
strict lint/consumer regressions, authenticated job routes and the native
component-deployment integration. The latter imports/reads exact model artifacts
through the real API/DB, measures the actual source-job output through the
registered native handler, checks live Tool blocking and malformed/missing
artifact failures, and prepares/inspects the real canonical encoding-usage graph.
It preserves one ordinary global root. This is native candidate-usage evidence;
it does not approve/activate the new usage or qualify Generate, serving or caches.

That frozen core source build used literal-reference/v1: its model view contained
620,802 UTF-8 bytes; the two framed Ornith request probes each contained 144,778
tokens, with 144,767 common leading tokens for those two particular questions.
The measured request is larger than the historical 131,072-token serving setting
before any response/history reserve. A fresh read-only discovery on 2026-10-19
from brassclaw2 to the model endpoint still advertises 131,072 for the same model
ID (`/Volumes/SSDE/brassclaw/target/validation/prefix-runtime-discovery-a3co1off/observation.json`). This confirms only the reported upper limit, not
long inference or current KV/recurrent layout. Preserve the complete candidate
and report Capacity blocked until a compatible target/design is qualified;
Generate must not change the operator's inference deployment. Do not activate it by ignoring that limit,
clipping text or relabeling the configured/architectural context. A larger
architecture limit is not evidence that the deployment has that capacity.
The complete retained package is in that receipt directory's
`core-generation.json`; no source or failed artifact was removed.

The successor packaged **source-only** design selects the already supported
literal-reference/v2-deduplicated whole-unit renderer. It retains original source
bytes and every origin/range mapping and emits repeated complete units once;
it does not summarize unique content or change an accepted installed model design.
Old source snapshots continue to name their original renderer mode. The resulting
size/headroom must be measured again through the actual generation/encoding path
before a model-specific design is accepted. The successor also adds a separate
effective-template encoding usage: original and supplied-template identities
remain distinct, and template measurements never approve an adapter or claim
server injection/cache compatibility.

#### Effective-template compatibility and retained-run correction

The successor native integration in
`target/validation/prefix-registry-artifacts-tvsgm75_` correctly failed rather
than accepting a bad effective-template measurement. Preserve that failure and
its complete core-generation artifact. Its source-only v2 build measured 602,082
context bytes and 141,820 framed tokens; whole-section deduplication alone does
not make the mandatory core fit the historical 131,072-token setting.

The independent reproduction identified two Python/Jinja compatibility gaps:
sequence concatenation must remain indexable/sliceable, and `None is iterable`
must be false. MiniJinja's lazy iterable semantics are not interchangeable with
the existing injection wrapper's list semantics. A version downgrade was not a
solution: inspected 2.x releases also use lazy concatenation. The pinned
`vendor/minijinja-compat` source retains the upstream license/hash inventory and
an explicit opt-in eager-concatenation patch. The encoder environment separately
corrects the None test and retains Python JSON/numeric behavior. Do not change
the effective template bytes to conceal a renderer mismatch. Dependency/adapter
changes require new worker identity and affected target qualification.

Frozen receipt
`target/validation/prefix-registry-artifacts-33w6ysc2/accepted.json` covers four
engine tests, the offline full AutoTokenizer oracle, strict engine/worker lint
and actual contained-worker checks, including safe classified template failures.
The full 12-context injected-template reproduction additionally matches the
independent Hugging Face report exactly:
`target/validation/prefix-encoding-reproduction-xk42v_67/accepted.json`.
It measures 141,807 shared leading tokens across the tested tools, role/content
and thinking variants. These are offline/contained measurements, not a native
usage bootstrap approval, server cache/state proof or model answer-quality claim.
The corrected native DB/usage integration is now accepted in
`target/validation/prefix-registry-artifacts-pct_i_6f/accepted.json` (2026-10-19).
It also covers affected-library strict lint, the closed source-compiler fixture,
internal result checks, authenticated routes and the three standalone consumer
regressions. Its complete source-only v2 context is 602,078 bytes; original-template
requests measure 141,853 tokens each and the 12 effective-template contexts retain
141,840 common leading tokens (complete requests: 141,857–142,100 tokens).
These counts belong to that exact frozen source package, not future catalogue
content. The canonical encoding usages remain candidate definitions: direct
native dispatch plus retained-graph inspection is not closed bootstrap
qualification or activated Recipe execution.

The fallible retained-run source API and ordinary matched/No-Match path passed
separately in
`target/validation/prefix-registry-artifacts-tvsgm75_/retained-prefix-accepted.json`.
Missing/expired captures now stop prompt assembly; a run cannot fall back to the
latest active prefix. Captured actor/project correlation is checked. Durable
restart restoration and complete model/design/prefix resolver identity remain
separate prerequisites; a String-returning compatibility source is not that
complete resolver.

Controlled formatting experiments were retained without promotion:
`target/validation/prefix-dedup-counterfactual-3qwnax8e` and
`target/validation/prefix-fragment-experiment-5g_nm4ai`. The latter reconstructed
all original text but still measured 139,885 framed tokens. It is not a supported
renderer or an accepted design. Do not keep splitting/rewriting prose merely to
claim a fit: the plan requires literal prose/Python/YAML, safe strict-JSON-only
display projection and explicit coverage/capacity rejection when mandatory
knowledge cannot fit. A compatible larger context/model may be necessary.

#### Reusable measured-capacity comparison (2026-10-19)

`scripts/prefix/prefix_capacity.py` and its class-22 `capacity_draft` now provide
pure `assess_prefix_capacity(inputs)`. The component consumes complete canonical
native observation bytes, expected specification/artifact/effective-template
identities and an explicit retained limits reference with context/output/additional
input allowances. It rejects identity mismatches, partial observation shapes,
noncanonical JSON, disabled full-token measurement, invalid counters, booleans,
nulls and negative allowances. It retains worker/preparation/template identity,
complete maximum request count, common-prefix count and fit/headroom/shortfall.
It never picks limits, drops knowledge, changes a model design or grants approval.

Frozen receipt `target/validation/prefix-registry-artifacts-ceyqqz_1/accepted.json`
executes that exact component in the real contained Monty worker using the native
12-context observation from the real source job and model store. The fixture's
131,072-token comparison fails fit; its explicitly hypothetical 262,144-token
comparison fits. Neither case establishes a deployed context limit, cache reuse
or answer quality. Template/artifact substitution, boolean context, null output
and negative additional-input cases stop. The same batch passes strict affected
library lint, compiler/result fixtures, authenticated routes and native candidate
integration. The component remains a packaged draft pending workflow/bootstrap
integration, not an activated Generate path.

#### Shared template adapter and preload acceptance (2026-10-19)

The offline `inject_reference` implementation now lives in the pure
`scripts/prefix/prefix_template.py` module, re-exported by `prefix_evidence.py`.
Its class-22 `template_adapter_draft` exposes `build_prefix_template(inputs)`;
there is no host call, serving edit or inference process. Offline compiler
procedure snapshots retain the new helper and the base compiler records/verifies
its dynamic `template_adapter_sha256`. Old archived packages remain readable.
Keep the entire compiler directory when moving offline scripts between machines.

Monty and CPython return byte-identical effective templates for the full retained
reference (`target/validation/prefix-template-component-i_u3awfl/accepted.json`).
The independent oracle now also requires the complete literal reference at the
start of every rendered request, rather than comparing token hashes alone.
It rejects a deliberately incorrect reference; all 12 retained contexts pass
(`target/validation/prefix-full-reference-oracle-ok9vbgv7/accepted.json`). The
model template trims trailing separator whitespace in some empty-system cases;
this does not remove reference bytes and is not a promised stable separator.

Native receipt `target/validation/prefix-registry-artifacts-gg8vy9cz/accepted.json`
passes both pure components' real preload inspection and execution, the exact
native model store/encoding path, complete-reference and HF checks, explicit
negative cases, affected strict lint, payload-serialization capacity regression,
three standalone consumer regressions and authenticated routes. Its parent
`prefix-registry-artifacts-v9nwiqdb` separately passed the four engine tests,
full offline HF oracle, contained worker and both dependency-boundary checks;
that parent's full native run failed on the subsequently corrected preload
issue and is not a full native acceptance receipt. Preserve both failed runs
(`ej4azlex` and `v9nwiqdb`) as failure evidence.

The preload loader currently rejects default arguments, including immutable
ones. `capacity_integer` therefore receives an explicit lower bound at every
call. Successful utility evaluation alone had not established loadability;
retain actual preload inspection in the candidate acceptance path. This change
does not loosen the loader, approve a Skill combination or activate Generate.
Native request serialization also counts escaped JSON bytes against captured
worker/frame limits before allocating another complete request string; oversize
requests fail as resource errors without truncation or an attempted child process.
Native-library memory containment remains a separate activation prerequisite.

The next candidate workflow is `prefix_measurement_components::measurement_drafts`:
ten explicit Recipe steps compose canonical JSON-digest and template-encoding
usages with pure parsing, template preparation, identity verification and capacity
comparison. It verifies that the supplied original template belongs to the model
artifact observation, and checks the effective template against its independently
computed digest. The Recipe serializes its exact calculation as the nonempty string required
by internal result-only completion; it does not publish a chat reply or qualify
as an ordinary chat route. Its owner must verify the resulting receipt.
It does not choose accepted
designs, validate deployed limits, bless cache/quality or publish a generation.
It still needs retained-graph/behavioral acceptance, closed first-party usage
qualification and integration with the pinned generation job before exposure.
The first graph test rejected a result-root `required` field and then a producer
that did not guarantee the encoding consumer's nonempty strings. Correct the
producer/result declarations; never weaken native validation to make composition
pass. Keep the fast full-graph/preload check ahead of native model measurements.

The ten-step candidate now passes full retained-graph/preload inspection in
`target/validation/prefix-registry-artifacts-9dbqe6sy/accepted.json` (frozen source
`prefix-registry-source-xpnr6fsq`). The same scoped batch passes strict affected
and standalone lint, native model-asset/store/handler and full-reference HF
checks, three standalone consumer regressions, route contracts and the ordinary
matched/No-Match single-root regression. The formatter input needs explicit
`required` and `checks` declarations; the pure-component constructor validates
input/result declarations before producing drafts. This establishes loadability,
not execution/approval of the complete ten-step measurement workflow.

The registration slice uses the existing primitive seeder for
`model_encoding`, including native-image verification, preservation of legacy
UUIDs/operator metadata and current blocks. It leaves the Tool pending and does
not approve or activate usages. Its registration/upgrade tests pass in the
`9dbqe6sy` scoped receipt above, including preservation of existing native
identities, metadata and operator blocks.

For closed bootstrap qualification, reuse `AcknowledgedPrimitiveSettings` and
`HostPrimitiveSettingsSource`. Source inspection and model encoding share that
same settings owner and one retained preparation worker; do not add another
bootstrap settings table, provider pool or per-model preparation supervisor.
The installation owner selects and validates startup limits, binds the contained
preparation backend before component qualification, and transitions that shared
owner once to the actual acknowledged live view before ingress. Each operation
captures one limits revision; candidate data cannot choose a worker or raise its
limits. Verify startup preparation, prohibited rebinding, live revision uptake
and ordinary matched/No-Match single-root behavior. This lifecycle slice passes
the `9dbqe6sy` scoped batch; it is not itself model-usage approval.

Implement the encoding usage's noncircular system-seed qualification separately:
exact packaged source/associations/native images, Q1 intent before behavioral
Tool calls, deterministic closed tokenizer/template fixtures with independently
known token-ID/count/hash outcomes, actual store/binding/kernel/dispatch evidence,
negative/policy/resource cases, durable effect accounting and exact immutable
approval records. Fixture model/provider records must be unmistakably private
qualification data in the existing owners, not active consumer selections or a
second registry. The selected preparation owner must already be usable for this
pre-readiness qualification; do not bypass its limits or invent caller-provided
success evidence. Qualified primitive usages do not establish real-model HF
compatibility, deployed context capacity, hybrid cache or answer quality.
Preserve incomplete qualification intents and fail closed without replay.

The closed owner is now implemented in `bootstrap_encoding_approval.rs`, using
`prefix_encoding_candidates.rs`, `bootstrap_encoding_fixture.rs` and
`bootstrap_encoding_validation.rs`. Its scoped behavioral acceptance is
`target/validation/prefix-registry-artifacts-8_yma761/accepted.json` (source
`prefix-registry-source-om94oqsa`).
It retains both exact canonical usages, verifies native and worker images,
records group Q1 intent before dispatch, executes the actual global Python
Recipe path and rereads its immutable selection/attempt/answer journal before
writing or reusing approval. Fixtures have explicit private, data-only provenance
in the existing model/provider/artifact stores; their imports remain unqualified.
Operator imports cannot supply this origin or approval fields. Original and
supplied-template cases include known full token arrays despite saved
padding/truncation settings, duplicate determinism, policy rejection, invalid
identities/assets/tokenizer/probes/template and render-fuel exhaustion.
Proof re-reading and incomplete-intent checks must establish no additional
dispatch. The existing source-compiler qualification shares the same contained
Recipe transport builder; it cannot increase selected value/frame bounds.
Keep these usage proofs separate from whole measurement-Recipe qualification,
native-library memory containment, model/design capacity, HF compatibility,
cache correctness, held-out model behavior and production Generate activation.

That same scoped batch executes the complete ten-step measurement candidate
through the actual global Python Recipe path: eight executable steps plus two
binding steps, exactly two native dispatches, the full ordered report equal to
the independent HF-verified report, and the exact internal capacity receipt.
It checks actual journal selections/arguments/answers, one attempt per dispatch,
child containment/reaping and no new approval evidence or chat reply. The full
source reference remains retained and capacity-blocked at the fixture's 131,072
context limit. This is development execution evidence, not a production workflow
approval or target/cache acceptance. The batch also passes strict affected and
standalone lint, three standalone consumer regressions, routes and ordinary
matched/No-Match single-root behavior.

Preserve earlier `xd_ryxoy` and `fom8q98m` failures: the old test counted all
artifact sets globally and rejected the two additional private data fixtures.
The corrected check counts the operator namespace's bindings; the authenticated
catalogue still must return exactly its one imported set. No fixture or evidence
was deleted to make that check pass.

The next retention slice, `prefix_measurement_candidates.rs`, gives this workflow
one installed stable graph, reusing the canonical digest and supplied-template
encoding identities instead of allocating another pair of usages. The source
compiler shares the digest identity allocator. Verify that the installed graph
matches both existing leaf approvals and retains its separate unapproved workflow
state, then qualify its complete pure logic and integrate the selected model/design
and generation job. Retention/inspection alone must not enable Generate.

That installed-graph retention slice passes
`target/validation/prefix-registry-artifacts-zxqty2cv/accepted.json`, source
`prefix-registry-source-cyg4wek4`, including execution of the actual installed
candidate and both canonical combination checks. The same scoped batch passes
native/HF checks, strict affected and standalone lint, standalone consumers,
routes and ordinary single-root behavior. Its two leaf associations remain
separate from whole-workflow qualification.

The new whole-workflow owner is `bootstrap_measurement_approval.rs`, with closed
fixture/validation helpers and storage-only `bootstrap_workflow_evidence.rs`.
It reuses `reborn_workflow_review_evidence` rather than inventing association
proof for pure logic or another approval store. The private selection pins both
existing leaf approvals before Q1/behavior; complete reviewed packaged workflow
qualification remains distinct from target/model/design acceptance. Expected
cases include fit/nonfit, duplicate determinism, pure rejection before effects,
policy and asset failure, and original-template mismatch after both completed
native calls. Verify every actual step input/result and durable invocation, preserve
incomplete intents and prove readback never replays. This slice passes
`target/validation/prefix-registry-artifacts-3tx1j8xr/accepted.json` (frozen source
`prefix-registry-source-hl_i7tx8`): all eight closed cases, proof corruption and
no-replay checks, full-reference/HF execution, affected/standalone lint, consumers,
routes and ordinary single-root regression. This scopes qualification to the
packaged workflow; no normal Generate, model quality, native-library memory
ceiling or activation claim follows.

#### Retained model/design measurement jobs (2026-10-19)

The model-specific validation prerequisite now uses the existing immutable
`brassclaw_prefix_job_definitions` and ordinary durable turn admission with
`operation=measure`, `mode=target` (V171). This is an offline diagnostic operation,
not Generate, design acceptance, a provider-capability claim or serving activation.
The caller selects an immutable source generation, profile/model design proposal,
model artifact set and explicit complete chat probes/capacity assumptions. The
storage adapter resolves each existing owner and rejects a different profile,
namespace, source evidence-view or model binding. Original bytes stay retained;
no filesystem/latest-provider lookup or source trimming substitutes inputs.

The definition stores source/design/artifact identities and the exact probe hash;
large probe/reference/template data live in its existing immutable input capture.
A lost-response retry retains the first definition; altered requests conflict.
Before dispatch, reconstruct and compare the exact capture and revalidate the
packaged whole-workflow proof and selection. The qualified ten-step Python Recipe
owns template preparation, measurements, identity checks and capacity arithmetic.
The host only admits, binds, enforces live policy and verifies the actual receipt
and immutable two-invocation journal. Completion stays an internal result with
Unknown/unverified quality/cache state; it writes no design acceptance or serving
selection. The source generation can remain quarantined or capacity-blocked.

`POST /api/webchat/v2/prefixes/{name}/jobs/{job}` accepts a measurement request
with a caller-chosen canonical non-nil UUID; the existing GET of that URL reports
progress/result and existing run APIs own cancellation. Keep authenticated route,
retained scope, no-replay, incompatible-input and full-reference/HF regressions.
The native production-path regression now passes in
`target/validation/prefix-registry-artifacts-62egl9f8/native-registry-export.log`
(frozen source `prefix-registry-source-qxi5edxv`): authenticated POST, retained
subjects, exact full-reference/HF result, two actual dispatches, duplicate POST
without replay, conflicting-request rejection, no reply or design-acceptance
write and unchanged global root. The complete scoped batch passes
`target/validation/prefix-registry-artifacts-62egl9f8/accepted.json`, including
strict affected/standalone lint, standalone consumers, route contracts and the
ordinary matched/No-Match single-root regression. The supplied full
reference requires over 142k input tokens before output/additional allowances and
fails the explicit 131072 context assumption. No content was dropped or deployed.

The generation-history UI now includes an Offline measurement panel with explicit
proposal/model-file selection, context/input assumptions, exact chat probes,
correlated job status and result. A failed POST retains the UUID/request/revision
for retry; session storage keeps only the job UUID. New-request editing is
explicit and does not cancel the old job. Frontend state regressions check retry
identity, rejected numeric coercion, model/profile filtering and terminal polling;
this is separate from browser/embedded-product acceptance.

This does not complete accepted-design selection, normal model-aware Generate,
Sources/Redesign/Create, domain migration, complete library/release coverage,
native-library memory containment or target/cache/held-out answer-quality
acceptance.

#### Declared release-document package (2026-10-20)

`scripts/prefix/release-reference-inventory.json` uses the existing offline source
inventory schema and compiler adapter. It declares root Markdown (excluding the
private local test configuration), `docs/reborn/` Markdown and crate agent/spec
maps, plus the packaged authoring/design references. The initial 201-document
inventory selects 13 references; unreconciled material is archived as
`development-only` and the explicitly superseded storage document is `superseded`.
These are conservative source dispositions, not a statement that every subsystem
has been reviewed or that all release documentation/private workflows are covered.
The accompanying inventory procedure declares the exact scope and remaining gaps.

`target/validation/prefix-release-document-package-k_cyzsry/accepted.json` verifies
two identical offline snapshots through the existing adapter, all 201 original
byte hashes and the same tested source/catalogue snapshot. The result contains
218 sources including catalogue records. This establishes reproducible package
integrity, not installation uptake, content quality or serving readiness.

The installed follow-up extends the existing class-17 reference revision owner
with compile-time embedded declared documents, exact inventory membership and
byte verification. It creates no new document/provider/approval registry and
performs no checkout reads at runtime. Existing core records and compatibility
projections stay distinct: extra archived/review-pending documents must not enter
`reborn_docus` as validated retrieval rows. Preserve operator edits and old exact
revisions through the existing revision store. Core-only source capture retains
its explicit nine-document selection; the declared-release capture selects the
broader package, preserving dispositions and original bytes. Both reuse the same
source compiler and durable job path, with quarantined output and no activation.

Installed acceptance passes
`target/validation/prefix-registry-artifacts-mn40lntp/accepted.json` (frozen source
`prefix-registry-source-b85d9bsh`): stored/compiled equality, stable distinct core/
release selections, no additional legacy projections, complete 6,339,248-byte
release snapshot compilation through the actual global orchestrator, five actual
native calls, exact archive receipt and reconnect without replay. A release request
cannot reuse a core-only job UUID; its immutable request records the distinct
capture scope. Strict affected/standalone lint, consumers, routes, HF/core
measurement and ordinary single-root behavior also pass. This does not close
missing private-library export, remaining source reconciliation/domain migrations,
per-model design acceptance or the normal Generate/serving cutover.

#### Version-aware qualified prefix workflow sources (2026-10-20)

The next source-export scope reuses the existing catalogue source owner, lock,
repeatable-read verification and DTO. The existing `/1` ordinary export remains
monoversion. The `/2` transfer additionally includes the already-qualified source
compiler and complete measurement selections; source visibility neither advertises
a callable nor activates a Recipe. Their owners must verify their actual
qualification before exporting. Missing qualification fails the expanded export,
not a partial-success claim. Other private workflows remain outside this scope.

An expanded transfer can contain different retained revisions of one stable UUID
across independent approved selections. Key records by UUID/version, verify exact
class/checksum/bytes and retain each selection's own closed dependency combination.
Reject conflicting bytes for one revision, class changes for one UUID, mixed
versions within a selection, missing dependency sources and unused source records.
Do not synthesize a latest combination. Keep version-specific logical source paths
and complete original transfer evidence. The separately retained global root has
no component dependencies today; future versioned root dependencies require an
explicit selection contract rather than guessing their revisions.

The offline adapter accepts both declared transfer contracts and keeps transfer
integrity separate from approval. The full native batch passes in
`target/validation/prefix-registry-artifacts-yvy49l3l/accepted.json` (frozen source
`prefix-registry-source-dxoiu78a`): actual `/2` export, both qualified private
selections, complete 250-source/6,602,089-byte release compilation, five native
calls and reconnect without replay. HF/core measurement, affected/standalone
lint, consumers, routes and ordinary single-root behavior also pass. Eight offline
importer/compiler tests pass against that emitted transfer, with one tokenizer
case skipped. The observed catalogue has no multi-version UUID; focused parser
mutations and the separate collector regression cover revision conflicts without
claiming those mutations are approved executable combinations. The independent
collector check passes in
`target/validation/prefix-registry-artifacts-2zj4l9zc/accepted.json` (frozen source
`prefix-registry-source-eqd7oucr`), explicitly scoped to source-transfer records
only; it is not another production-path qualification. The declared-release capture uses the expanded
scope; the core-only capture and existing public ordinary-source route retain
their narrower contract. This does not complete all private-library coverage or
model/quality/activation acceptance.

#### Versioned trailing-inventory layout (2026-10-20)

`literal-reference/v3-trailing-inventory` reuses the exact-unit renderer and
`brassclaw-prefix-render-map/1`, retaining complete original bytes, line ranges,
content-derived evidence identities and per-source dispositions. It moves the
volatile source inventory after all complete reference cards. This keeps the
leading reference bytes unchanged for inventory-only edits without hiding the
changed source decision or its generation identity. It does not guarantee stable
leading tokens after content edits or changes to shared-unit grouping.

The existing Recipe-owned compiler stages, offline adapter/worker, closed model-
design proposal schema and archive validator recognize the explicit option.
Existing defaults and accepted designs are unchanged. Qualify both profiles with
independent closed rendering oracles, exercise the option through the actual
retained global source job, preserve v1/v2 byte equivalence, and compare actual
model/template token sequences before selecting this layout for a model. This is
one formatting prerequisite; it does not solve JSON-escaped embedded programs,
capacity, native-library memory bounds, model quality or serving activation.

Offline exact-token evidence is retained in
`target/validation/prefix-layout-token-stability-lg9kms6d/evidence.json`.
Using the retained full core source package, exact local tokenizer/template and
one plain chat, changing only an excluded source's disposition reason preserves
311 leading tokens under v2 versus 140,773 under v3. Total input counts are the
same for both layouts (141,768 before, 141,774 after). Original evidence/maps stay
identical and v2 rendering equals the retained original context. This is one
metadata-mutation/tokenization observation, not inference, provider-envelope/cache
qualification, held-out answer quality or a claim that the full reference fits the
explicit 131072 context assumption.

#### Progressive stable-sharing layout (2026-10-20)

`literal-reference/v4-stable-sharing` uses the same exact original/range map and
trailing inventory. Its first occurrence of each complete unit remains literal;
later occurrences point to that original source/range. Unlike v2/v3, discovering
a later duplicate does not change how the earlier occurrence is displayed. No
near-duplicate matching, summary or clipping is introduced. Canonical source order
still matters: earlier insertions and content edits can change earlier tokens.

An offline regression appends a complete exact duplicate of a mandatory guide.
All earlier reference bytes stay identical under v4, whereas v3 changes earlier
sharing labels. Every mapped source, including the appended duplicate, reconstructs
byte-for-byte from its original units. The shared compiler, offline worker,
archive/design contracts and independent closed fixtures recognize the new option;
existing view semantics/defaults remain unchanged. Bootstrap now has twelve closed
cases, checked by exact case names rather than the obsolete integration count of
eight. V3's batch `prefix-registry-artifacts-e0tug_1k` completed source jobs and
closed oracles but failed that stale count assertion; preserve it as failed batch
evidence. A successor must pass the full required native checks before claiming
qualification. These layouts do not establish accepted design, context fit,
provider/cache behavior, source quality or model accuracy.

The full successor batch passes in
`target/validation/prefix-registry-artifacts-273hlb17/accepted.json` (frozen source
`prefix-registry-source-99sk8rmh`): all twelve closed qualification cases, both
profiles/four independent rendering oracles, actual v4 global source job, complete
250-source/6,613,880-byte declared-release job with five calls and no replay,
retained measurement/API/HF checks, strict affected/standalone lint, standalone
consumers, route contracts and the ordinary single-root regression. The source
jobs stay quarantined; installed profile defaults remain v2. This covers the
frozen integration, not subsequent concurrent provider/model changes.

The retained offline update comparison is
`target/validation/prefix-layout-update-token-stability-4ik5nu12/evidence.json`.
It uses the exact same retained core source and local tokenizer/template, one plain
chat and three explicitly experimental mutations (reason-only, appended complete
exact duplicate, late-guide newline). Earlier v2/v3 rendering remains byte-identical
to the previous renderer for all cases. Token-prefix observations are:

| Layout | Reason-only edit | Appended later duplicate | Late-guide newline |
| --- | ---: | ---: | ---: |
| v2 | 311 | 1,282 | 80,584 |
| v3 | 140,773 | 189 | 79,430 |
| v4 | 140,346 | 140,164 | 79,003 |

V4's unchanged full input uses 141,341 tokens versus 141,768 for v2/v3; the lower
late-guide token position reflects this shorter earlier rendering, not a cache
regression at a different source. Appending the duplicate uses 144,881 tokens in
v4 versus 148,078 in v2/v3. All exceed the explicit 131072 context assumption;
these are complete offline token observations, not approved content changes,
serving hits, inference quality or evidence that model capacity was increased.

### Phase C1 — Sempai reference, review packet and component proposals

Register/seed the Sempai profile and immutable compiler/source/procedure artifacts.
Extend the shared compiler adapters and Recipe jobs to generate it and the
base-plus-Sempai composite. Wire the typed review packet and actual reviewer persona/
response schema through the existing interceptor ports, forensic storage, gateway
and proposal sink. Preserve input identity, target-provider metadata and effect
history. Keep pre-call review and post-response learning separate and avoid a
parallel agent loop. Use the existing class-specific draft/Q1/Q2 paths; implement
missing association/constructor/provenance support under Gate A1 rather than hiding
it in a prompt. Production reviewer activation also requires Phase C2 acceptance.

Acceptance: six seeded profile definitions; complete source/procedure provenance;
deterministic Sempai rendering with no HA leakage; capacity failure without dropped
guides; and dedicated Sempai consumer selection independent of Kohai. Through the
production caller, test unchanged prompt echo, safe targeted prompt adjustment,
malformed packets, missing provider capabilities, structured tool-call/result IDs,
injection attempts, secret redaction and no loss of user constraints/effect history.
Use representative anonymized conversations to propose a reusable Recipe and
associated usages with actual supported schemas/draft identities. Verify Q1 failure,
pending human Q2, rejection, exact-combination approval and coherent activation;
no direct validated writes/self-approval or unsupported fields. Verify old tasks
retain their selection, failed steps never replay as Tier 2, the production reviewer
schema is not replaced by citation-benchmark JSON, and Tier-0 tasks gain no hidden
Sempai calls. Prompt improvement/proposal correctness needs actual case evaluation,
not only token/cache measurements.
Port the whole-artifact validator through the real lossless sink; include missing/
null/default/boolean/list/recursive/bounds/helper/result cases and malformed/duplicate
Recipe layouts, invented Tools/UUIDs and summary/payload disagreement. Add independent
transfer cases with different domains/field names/bounds, hold them out of teaching,
and report first-pass separately from repair acceptance. Verify grammar/schema
fingerprints and host-authorized edit constraints without forcing every Recipe to
one variant. A rejected candidate stays rejected, even if its summary claims success.
Test artifact-first/summary-last decoding through the actual production consumer,
including rejection of old reviewer order even when a canonical sorted fingerprint
matches. Regenerate incompatible planner grammars; test exact quote provenance,
missing requirements, erroneous artifact classification and invented defaults/branches.
Verify invocation audits cannot force effectful loading or module-level result
assignment into preload definitions. Validate empty proposal arrays against the
requested artifact, not against compatibility arrays or optimistic summaries.
The six v25 protocol tests and source/compiler checks are standalone development
evidence; production sink/loader acceptance and fresh first-pass/transfer/cache-path
model qualification remain prerequisites, not inferred improvements.

Validate current Skill preload definitions and their invocation artifacts separately
through the selected production Monty host/ABI: effect-free loading, dependency
order/export resolution, isolated mutable state, complete returned results and
retained task selections. Upstream Monty pure-logic probes do not certify that loader,
kernel binding or private transport. Include current MCP discovery/ordinary-chat
contracts as source-backed knowledge checks, without inventing an external Python
execution API or treating documented targets as implemented support. Include correct
answers with unrelated valid citation IDs and wrong-but-verbatim quotes as failures;
source resolution, quotation fidelity and semantic support remain separate gates.

### Phase C2 — Minimum provider resolver before activation

Wire immutable selections into Kohai and Sempai independently. Implement task and
continuation pinning, atomic freshness admission, explicit injection mode,
duplicate-prefix prevention and base-plus-domain composite selection/capacity checks.
Include the base-plus-Sempai reviewer composite and its separately resolved provider.
Enforce the server-owned endpoint contract above and retain old routing bindings
while pinned tasks need them. Implement activation/rollback and minimal warm/reuse
operations through the existing facade and provider seams before enabling their UI.
Require the DB-only provider contract and its relevant migration/reload acceptance
before this phase's production gate; no prefix-specific provider registry is allowed.

Acceptance: production model requests use the selected generation exactly once;
source activation races block/queue new tasks rather than serving outdated context;
ordinary replacement preserves old continuations; revocation is classified; mutable
server templates cannot change an admitted request's generation. Test restart,
concurrent activation, both consumers, incompatible/capacity-blocked composites,
effective injection/capacity evidence and adapter-specific exact token observations
where available. Require a second-request native reuse observation only for adapters
declaring observable reuse; adapters without telemetry must pass inference/pinning
acceptance with Cache reuse unverified. `/tokenize` alone does not satisfy
completion-time generation enforcement. Gate A1 and Phase C
must pass before enabling this production resolver.
Test DB-only targets without file/Env fallback, provider URL/model/context changes,
revision-aware reload, deactivation/removal including the last provider, continuation
dispatch after deactivation, explicit reactivation without restored selections,
DB failure versus Unconfigured, and usable operator management/Tier-0 paths.
Test a provider without cache telemetry and ensure activation/inference remain
available while unsupported warm/verification actions and observations are honest.
For targets serving hybrid external hits, section 9.1 is an acceptance prerequisite,
not work deferred to Phase E: exercise cold/native/external/restored-native paths,
plain and constrained decoding, expected content and representative semantic probes.
Keep failed cache experiments out of model scores. Check bad page mapping, absent
recurrent checkpoints, changed backend/chunk/source build and previously corrupted
native state; known failures block the target. Qualify the actual configured path
without new random salts concealing it. Package upgrades invalidate backport-bound
receipts. No service/package/deployment mutation occurs from a WebUI build/warm job.

### Phase C3 — Prefix-Maker, Redesign and custom profile creation

Seed the reviewed Prefix-Maker package/design and register its model-free compiler.
Implement section 5.3's DB model specifications/runtime identities, reusable model
designs, profile/consumer bindings, immutable proposal storage and migration/retention
contracts. Implement compatible shared-engine options and exact acceptance contracts.
Author the Tier-1 Redesign/Create/Generate model design Recipes,
reusing existing provider, export, validation and proposal primitives. Production
calls require Gate A1, Phase C/C2, usable Prefix-Maker and the lossless proposal path;
do not activate unsupported workflows or route failures through Tier 2.

Acceptance: bootstrap without provider/self-reference; exact Prefix-Maker injection
once; distinct design-provider/target identities; pinned input and schema references;
confirmation bound to the actual request; raw proposals and full diffs preserved;
real trial builds; invalid/unknown options rejected; mandatory coverage preserved;
capacity/missing architecture surfaced; no operational deployment changes or arbitrary
compiler execution. Check malformed/provider-failed/cancelled/duplicate requests,
recovery after a possible provider effect, stale proposal acceptance, custom-profile
identity conflicts, human rejection and Prefix-Maker self-redesign with the old
approved generation. New design selection never activates a generation or bypasses
authored Q1/human Q2. Redesign must collect current approved content/architecture and
produce a complete updated generation; acceptance publishes the exact validated
candidate together with its design evidence, without another manual Generate.
Identical approved model-specific design and source inputs reproduce output; new
approved components change content while keeping the selected design contract.

Exercise two real registered models with different tokenizer/template/context and
architecture capabilities through the production DB/provider path. Verify separate
formatting/capacity designs and profile bindings, Unknown model/cache facts, reusable
compatible defaults and incompatible overrides. Test design-provider versus target
model separation, Kohai/Sempai provider/model changes independently, same-name/different
weights, template/context/backend edits, missing design/prefix readiness, staged switch,
concurrent acceptance/switch and restart persistence. No stale incompatible generation
serves new work, no hidden LLM design call occurs on Generate, and old tasks/history
retain their references. Cache-only changes requalify runtime evidence; do not rebuild
identical reference content needlessly. Seeded/custom model designs survive upgrades.

### Phase D — WebUI vertical

Wire facade/HTTP/descriptor/runtime/frontend changes together. Show seeded and registered custom profiles,
durable progress, generation details and independent Activate/Warm states. Update
translations and keyboard/accessibility behavior. No browser-triggered arbitrary
script, source path or environment execution.
Include composite job admission/detail controls and parent/target dependency status
within the domain/reviewer rows; activation selects a published eligible composite explicitly.
Show the Sempai row's source quality, reviewer target/readiness and proposal
validation guidance. Keep review enablement separate from reference generation.
Show Prefix-Maker readiness, Redesign beside each Generate button, the confirmation
popup, proposal/diff review and Accept redesign controls. Add the Create Prefix tab and
custom-profile draft/registration lifecycle through the same facade/API/runtime path.
Add Model Designs selection/generation/list/review/binding controls and affected
consumer readiness on provider/model changes; expose persisted historical designs too.
Implement section 8.1's Sources button/tab, manifest-backed inventory, durable source
registration/check workflows and per-source availability projections. Wire existing
fetch/artifact/store adapters through Recipes; implement genuinely missing support
in its owning layer before exposing enabled actions. Source checks/registration use
accepted Gate A1/C paths and do not require a design provider; Redesign-based collection
still follows Phase C3, while offline approved-snapshot Generate remains available.
Generate requires Gate A1 and Phase C acceptance. Activate/Rollback/Warm requires
Phase C2 acceptance for the specific registered provider target. Until then, show
disabled actions with their missing prerequisite; enforce the same gate server-side.
Redesign/Create/model-design generation/acceptance require the accepted Phase C3 path; their absence must
not disable the model-free Generate workflow.

Acceptance: real composed-route tests and browser checks for first Generate,
Regenerate, refresh/reconnect, cancellation, per-profile failures, unavailable
worker/provider, rate limiting, stale source and compatible/incompatible activation.
Navigating away must not lose a job. A failed build must not replace an active prefix.
Cover generated-parent versus ready-composite state, composite build/rebuild,
dependency/capacity failures and a valid provider lacking cache telemetry.
Cover the section 1.1 completion states through the browser and composed API: one
click builds eligible parents/composite, repeated clicks deduplicate, prerequisite
and quality/cache failures stay distinct, advanced details expose omitted-unit
reasons and receipts, and Generate never changes the effective provider selection.
Exercise popup cancel/Escape (zero admission/provider calls), explicit confirmation,
changed prerequisites, double-submit/reconnect, provider unavailable, stale/rejected
proposals, new-profile creation and custom profiles surviving reseed. Verify keyboard
focus and accessible dialog/tab labels. No button or popup implicitly accepts a model
proposal, approves a component or activates a prefix. Verify active-model selection,
separate design-provider selection, generated model-design history after reload,
validation/acceptance and explicit bindings, switch readiness for both consumers,
and Redesign returning a complete current-content generation rather than design only.
Test Sources on every seeded/custom row, inherited composite sources and pre-generation
registrations, immediate inventory plus per-source checks on opening, green/red/neutral
accessible states, check-again/dedup/reconnect and observation persistence. Exercise
real supported HTTP/content and DB/artifact adapters: redirects, soft 404/login/empty
pages, missing retained artifacts, timeout/rate-limit/auth/policy denial and full-versus-
partial content identity. Show an intact archive despite upstream failure. Save a new
source, reject credentials/unsupported schemes/conflicting duplicates, verify restart
persistence and Pending review, then prove inclusion in the next eligible Redesign or
Generate without changing a running build. Test Needs redesign, capacity/exclusion
reasons and no hidden LLM/source execution or automatic active-generation replacement.

### Phase E — Extended cache and answer quality

Build on the accepted Phase C2 resolver. Add richer cache observations and optional
external persistence adapters. Integrate citation validation in actual answer
routes and retain domain-specific config checks; minimum provider correctness is
already a prerequisite for Phase D activation, not work deferred to this phase.
Mandatory source/artifact/teaching/configuration checks are part of the build gates
above. This phase broadens measured model/answer quality; it does not postpone those
checks or reinterpret a compiled artifact as proven expertise. Quality evaluation
is an explicit separate Recipe/job using a quarantined immutable candidate and an
accepted injection adapter, never an implicit compiler call or production activation.
It must not require an already active candidate to test that candidate or create
a first-build dependency cycle. No provider means model quality is Unmeasured.

Acceptance: exact effective `/tokenize` or equivalent adapter evidence; no duplicate
automatic/client prefix; pinned selection across concurrent changes and continuations;
context-capacity failures without silent omissions; cold/warm native measurements
only where supported, and separate external reload proof for
adapters claiming it. Unsupported cache observations remain Unverified rather than
blocking otherwise valid providers. Test source-backed component/Recipe
questions, security maintenance, HA YAML/MQTT/Modbus and tomedo API/version cases,
including unavailable functionality, fake schemas, backdated billing versions,
forged/unrelated citations and configuration that parses but is invalid. Compare
accuracy as well as cache reuse; latency/cache hits do not establish expertise.

## 12. Validation scope, rollout and stopping rule

This planning change requires source/diff/link review only. No compiler build,
database migration, provider deployment, prefix activation or remote service change
is performed by creating this document.

Implementation uses focused checks under the development policy: real PostgreSQL
for storage/job behavior, real Monty/Recipe dispatch for execution claims, affected
facade/route/architecture tests for wiring, and browser verification for the UI.
Use `LOCAL_TEST_ENV.md` before any remote/provider test. Preserve model deployments
and reuse passing evidence until relevant changes invalidate it.

Roll out behind the existing base-store compatibility resolver. Import verified
domain bundles without rebuilding them, but label unverified legacy artifacts.
Keep old generations/selections and rollback references until active tasks and
continuations release them, including generation-specific provider routing bindings.
Compatibility reads do not authorize new tasks to consume outdated/unverified
context under the final resolver. Gate A1 precedes production Recipe jobs; Phase C2
production acceptance precedes enabled activation and resolver cutover. Remove the
obsolete formatter/store path only after all consumers are
migrated. Update current docs, seeds and checksums together through supported paths.

### Coordinated upgrade and recovery

Use simplified-v3 Phases 6 and 8 for installed schema/binary cutover, with exactly
one management/migration owner. The required sequence is:

1. Run a configuration/data dry-run: inventory legacy bundles, provider selections,
   source/approval provenance, scope-key collisions, running jobs and task/artifact
   references. Report ambiguities for operator resolution before data changes.
2. Stop admission; drain or durably reconcile outstanding tasks/jobs/effects, then
   stop the orchestrator/workers. Do not claim old RAM/dump state is portable across
   an incompatible Monty or binary upgrade.
3. Take a consistent backup of DB, required source/compiler/Tool artifacts, provider
   routing references and protected key material. Verify a restore and reference/
   artifact availability before migration; never expose secrets in migration output.
4. Apply versioned, idempotent/resumable migrations through that owner. Preserve
   stable IDs, exact approvals, workflow/continuation links and invocation/effect
   records; resolve collisions explicitly. Verify source/selection/secret references
   and load the compatible binary/seeder. Run repair only when explicitly requested,
   preserving operator edits; historical migrations/checksums remain unchanged.
5. Verify component integrity and the Phase-8 component-cutover gate, then start
   the single global Monty to its Ready handshake before workers/turn ingress.
   Verify the DB-only provider catalogue and applicable prefix gates. No mixed
   old/new orchestrator operation or legacy permission fallback is permitted.
6. Perform authenticated management, Recipe, prefix selection and provider smoke
   checks. Only then enable accepted production actions and remove obsolete paths.

Keep Embedded-PG available under the migration owner's exclusive coordination for
backup/migration; stop it last during final shutdown. Read-only diagnosis and
authenticated management remain recoverable when a cutover gate fails.

Prefix-generation rollback changes an eligible selection under section 9's checks.
Binary/database rollback is a separate recovery operation: restore the verified
pre-upgrade backup and compatible binary/artifacts when the new schema is not
backward compatible. Document this procedure and test interrupted migration,
restart, reference/approval preservation and restore; switching a prefix pointer
does not undo a schema or Monty upgrade. This runbook governs BrassClaw upgrades,
not replacement of the operator's external vLLM/Compose deployment.

Completion means all six seeded profiles and approved custom profiles can be generated/regenerated through the UI,
base-prefix inputs contain the complete approved current docs/components, source and
generation history survive failure, model calls use the intended immutable context
exactly once, and quality/cache status is supported by real receipts. Nothing in
this plan authorizes claiming simplified-v3 runtime gaps are solved by prompt text.
Each profile also exposes confirmed provider-assisted Redesign, and Create Prefix
can produce a reviewed custom profile/design. These paths preserve existing designs
and active selections until their separate acceptance/activation operations.
Redesign produces a complete updated generation with current approved knowledge;
accepted per-model designs, size/formatting rules and profile bindings persist in DB.
Model Designs lists generated revisions and can generate a design for an active
registered model. Provider/Kohai/Sempai model changes use compatible designs/generations
or report their explicit readiness prerequisite rather than serving mismatched context.

### Final one-click acceptance matrix

| Operator scenario | Required result/evidence |
| --- | --- |
| Generate a profile for a registered consumer/target | One durable operation builds/reuses eligible parents and validates the complete composite; exact source/target references and downloadable receipts are visible. |
| Generate without a provider | A valid source artifact or explicit missing-local-artifact prerequisite; model quality/cache readiness remain Unmeasured/Unverified. |
| Repeat or reconnect during Generate | Durable deduplication, progress and retained child references; no repeated confirmed publication or automatic activation. |
| Change sources/target, cancel, or exceed context | Classified stale/blocked/cancelled outcome, preserved originals and last-good selection; no silent partial knowledge or unreviewed replacements. |
| Rebuild on another supported worker/path | Identical canonical content, evidence IDs, rendered text and leading tokens from identical pinned inputs. |
| Validate knowledge and teaching | Required coverage, complete evidence units, independently specified behavior checks and transitive validator receipts; first-pass/repair/transfer results remain separate. |
| Activate with enabled external caching | Accepted exact injection plus qualified real restore correctness; known-corrupt or incompatible paths block activation without changing the deployment. |
| Inspect an active generation | Desired/effective selection, freshness, capacity, model quality and cache observations are distinct and traceable; retained tasks keep their pinned revisions. |
| Cancel or confirm Redesign | Cancel makes no job/provider call; confirmation admits one pinned Tier-1 task, refreshes current approved content/architecture and returns a model-specific proposal plus complete updated candidate, without replacing the existing design or active prefix. |
| Accept a redesign | Exact reviewed model-specific design and its validated updated generation are recorded/published without another Generate click; later Generate reuses the accepted design and production activation remains separate. |
| Create a new prefix | Same-section Create Prefix tab yields a validated draft and human-accepted registered custom profile; it appears with Generate/Redesign and survives reseeding. |
| Redesign Prefix-Maker itself | Previous approved fresh generation reviews a quarantined successor; bootstrap and approval never depend on that unvalidated successor. |
| Generate model design | Active registered target and separate design provider yield a persisted, validated proposal with model-specific formatting/capacity rules; acceptance and profile binding are explicit. |
| Change Kohai/Sempai provider or model | Resolve exact compatible model/profile/consumer design and generation, or report Needs design/generation/Incompatible; no old mismatched prefix or hidden redesign call. |
| Reopen Model Designs | Generated and historical model designs, evidence, acceptance and bindings survive reload/restart and reseeding. |
| Open Sources | Manifest-backed used sources and pending registrations appear; bounded per-source checks report green/red/neutral with reasons and timestamps, without changing archived evidence or active prefixes. |
| Save a source | Versioned registration persists, is collected/reviewed for the next eligible Redesign/Generate, and remains visibly unused until included in a validated generation. |

Run this matrix through the production facade, composed ingress, Recipe worker and
browser path. Script-only success or a screenshot of buttons does not close the plan.

## Kohai final prefix stage and MCP provider lifecycle

After composing history/query and adding the selected complete prefix as the final
prompt composition step, Kohai obtains the coherent listing derived from approved
mcp-call-skill-recipes, establishes the provider's supported MCP connection and
advertises it immediately before sending the prompt. Do not reconnect by starting
a server or compile a prefix merely to establish that connection. The one MCP
server remains running during idle/disconnected periods. On the complete model
answer, clean up only this request's connection/advertisement; use the same cleanup
for failure/cancellation with owned in-flight calls reconciled. Streaming progress
is not completion. Refresh server discovery on startup/restart and qualified
Skill/Recipe activation; keep each advertised request contract and normal chat
execution snapshot coherent. MCP calls remain new ordinary chats, not direct
Monty/Rust invocations. See [the exact lifecycle contract](monty.composition.md#recipe-derived-discovery-and-kohai-owned-provider-connections).

This is lifecycle adapter planning only. Verify actual provider MCP configuration
and normal chat APIs before implementation; no endpoint/SDK field or runtime
acceptance is inferred from these instructions.
