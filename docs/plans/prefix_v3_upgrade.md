# Prefix v3 upgrade: compiler, source catalogue and WebUI

Status: implementation plan, not shipped functionality. Source and evidence review:
2026-10-07. This revision incorporates Sempai quality experiments and the reproduced
LMCache transfer failure; it does not deploy code or change inference services.
The workspace package version is `1.6.0`; release identity also needs a commit and
content manifest because version numbers alone do not identify the working tree.

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
  -> pin Prefix-Maker, existing design/source evidence and provider/target metadata
  -> provider proposes a new versioned design or new profile draft
  -> retain proposal; validate contracts and compile a quarantined trial generation
  -> show design/content diff and quality/capacity evidence
  -> operator reviews and accepts a validated design revision
  -> Generate uses that revision; Activate remains a separate operation
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
| `base-prompt` | BrassClaw Base Prompt | `base-promt-compiler.py` | Current release docs and complete approved component snapshot. |
| `defensive` | Defensive / Servers and Networks | `Defensive-compiler.py` | Existing defensive source families and security/maintenance evidence. |
| `homeassistant` | Home Assistant / MQTT / Modbus / YAML | `Homeassistant-compiler.py` | Existing official domain sources and coverage gates. |
| `tomedo` | tomedo / API / macOS / PostgreSQL / Billing and Coding | `tomedo-compiler.py` | Existing official docs, qualified forum observations and curated local findings; no clinical knowledge. |
| `sempai` | Sempai / Prompt Review and Component Authoring | `sempai-compiler.py` | Complete BrassClaw authoring/validation references and explicit prompt-diagnosis, Recipe and component-proposal procedures. |
| `prefix-maker` | Prefix-Maker / Prefix Design | `prefix-maker-compiler.py` (planned) | Versioned prefix engineering, evidence/formatting, model/tokenizer/template and cache architecture references, measured design lessons and the supported profile-design contract. |

Keep `base-promt-compiler.py` as the requested filename; the canonical profile name
is correctly spelled `base-prompt`. Paths are registry-owned, not browser-supplied.

Implementation begins by copying `Homeassistant-compiler.py` to
`scripts/prefix/base-promt-compiler.py`. Replace **all** HA-specific source lists,
domain/platform tags, coverage gates, routing patterns, headers, runtime guidance,
default cache paths and usage instructions. Add release-document and component-
snapshot input adapters. Do not leave a Home Assistant topic gate or system prompt
in the new base compiler. Do not rename or replace the existing domain compilers.

After obtaining behavior parity, extract the common evidence/assembly machinery
into a shared module. Keep small profile entry points and profile-specific source
adapters. Preserve independent caches, raw sources and historical generations.
Sharing a rendering engine must not mix domain source databases or approval rules.

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
| Evidence index | Stable IDs, original ranges/fields/revisions, original and rendered hashes, applicability and navigation. |
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
The current sink supports classes 21/22 but discards v3 Recipe fields; preserve
those fields through the supported constructor/sink path before accepting v3
Recipe proposals. See [the live test report](../../scripts/prefix/sempai-evaluation-20261006/README.md).

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

### 5.1 Knowledge quality and model-facing structure

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
trusted envelope, not duplicated personas inside factual source units. Only the
trusted envelope establishes authority; quoted source instructions remain data.
Clearly delimit reference data. Preserve complete programs, YAML blocks,
qualifications and related prerequisites together; nested source examples must not
become chat roles or new instructions. Do not summarize away executable artifacts
or repeatedly duplicate instructions. Stable source/component identifiers support
navigation and citation checks.

Teaching must transfer beyond a fixture: presence before value access, missing
versus null, exact booleans versus integers, recursive container/item constraints,
numeric boundaries, defaults only on missing inputs, complete invalid results,
helper invocation and module-level result assignment, typed step handoff, effects
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

Select content by required coverage and measured transfer, not “fill remaining
tokens.” Preserve the large prefix and full source store; admit optional whole
units only under a reviewed deterministic policy after mandatory units fit. If
mandatory knowledge cannot fit, fail with exact coverage/capacity evidence rather
than choosing lossy slices. A fully seeded base library may require a larger model.

### 5.2 Prefix-Maker reference and versioned design workflow

Introduce `prefix-maker` as the sixth seeded knowledge profile, independently
generated through the same model-free compiler and evidence gates. Register its
source package, procedure and planned `prefix-maker-compiler.py` entry point;
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

Compile a quarantined trial with pinned sources and the proposed design; require
evidence preservation, coverage, formatting, capacity and reproducibility checks.
Optional provider evaluations are explicit child jobs with first-pass/repair/transfer
results and cache correctness separated. Show Unsupported, Capacity blocked or
Validation failed rather than claiming an upgrade merely because a model approved it.
Present old/new design and content diffs, omitted-unit reasons and receipts for human
review. Persist the accepted design as a new immutable revision with review evidence;
authored component changes still require Q1 and human Q2. Selecting it for future
Generate is explicit and revision-checked, never automatic production activation.

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
| Design revision/proposal | Stable profile identity, immutable declarative design/schema revision and checksum, predecessor, pinned Prefix-Maker/provider/source/target references, raw proposal, diff, validation/trial receipts and acceptance evidence. Draft proposals never become active designs by a status label alone. |
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

Actions: Generate / Regenerate for each profile; View job / Cancel while running;
View sources and validation; Activate a chosen compatible generation; Warm /
Verify cache when the provider supports it; Roll back to a retained generation.
Generating a prefix must not automatically replace an unrelated provider's active
domain. Do not mark “fresh” optimistically from a job admission response.

Place a button labelled **Redesign** immediately beside Generate / Regenerate on
every seeded/custom profile row. Before admission, open an accessible confirmation
popup naming the profile/current design, design provider/model and target, permitted
source data sent to that provider, requested goal and the expected outcome: a draft
design for review, with no active prefix changed. Offer Cancel and Confirm redesign;
Escape/cancel makes no job or provider call. Revalidate prerequisites and exact
revisions server-side on confirmation. If they changed, require confirmation of the
updated request. Persist confirmed request identity/idempotency before dispatch so
double clicks, reconnects or recovery do not create duplicate confirmed effects.
This is the requested UX confirmation, not a per-Tool authorization lease; current
global policy, external authentication and technical limits still apply independently.

Redesign shows durable progress, raw proposal/diff, trial validation and a separate
Accept design action. Failed/cancelled/rejected proposals preserve the selected design,
generated artifacts and active selection. Accept design updates only the saved design
for subsequent Generate; a trial may be reused only when its exact accepted inputs
match. Source refresh is not Redesign, and Regenerate never asks a model to rewrite
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
| Observe/cancel | `GET /api/webchat/v2/prefix-jobs/{id}` and `POST /api/webchat/v2/prefix-jobs/{id}/cancel`; reuse runtime events where available. |
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
8 full-attention layers, AWQ INT4 weights, attention KV dtype and recurrent-state
dtype. Preserve that distinction: INT4 weights do not mean INT4 KV or recurrent
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
parser behavior. Protected-message/single-proposal/single-variant constraints apply
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
| Phase C3 | Accepted Gate A1/C/C2 paths plus immutable design/proposal contracts and Prefix-Maker bootstrap; authored executable changes follow Phase 0a Q1/human Q2 and registration. |
| Phase D | Phase 5 instance operator UI/API; reachable actions depend on accepted Gate A1/C/C2 paths, with C3 additionally required for design operations. Prefix knowledge profiles do not introduce security RuntimeProfiles or product editions. |
| Phase E | Phase 7 answer/cache acceptance and §5.1 performance evidence; quality and cache reuse remain separate measurements. |
| Production cutover | Phase 8 coordinated migration and component-cutover gate after the applicable prerequisite matrices pass. |

Reuse accepted upstream v3 work and receipts; do not reimplement its lifecycle,
provider catalogue or policy engine within the prefix feature. Independent offline
development need not wait for every unrelated v3 cleanup, but production paths must
pass all contracts applicable to their consumers.

### Phase B — Base compiler and release source package

Copy the HA compiler to `base-promt-compiler.py`, replace its domain content, add
document/component adapters, then extract shared compiler code with parity tests.
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
Also require host/path-independent content identity; byte/hash agreement between
source, evidence and rendered text; reproducible whole-unit selection; intact
YAML/Python/JSON; reviewed conflicting/versioned sources; tutorial mutation checks
and transitive validator fingerprints. Prove zero provider/network/process discovery
through the actual worker entry point. No optional synthesis mode is invoked.

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
Implement immutable declarative design/proposal storage, compatible shared-engine
options and exact acceptance contracts. Author the Tier-1 Redesign/Create Recipes,
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
authored Q1/human Q2. Identical approved design and source inputs reproduce output;
new approved components change content while keeping the selected design contract.

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
popup, proposal/diff review and Accept design controls. Add the Create Prefix tab and
custom-profile draft/registration lifecycle through the same facade/API/runtime path.
Generate requires Gate A1 and Phase C acceptance. Activate/Rollback/Warm requires
Phase C2 acceptance for the specific registered provider target. Until then, show
disabled actions with their missing prerequisite; enforce the same gate server-side.
Redesign/Create/Accept design require the accepted Phase C3 path; their absence must
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
proposal, approves a component or activates a prefix.

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
| Cancel or confirm Redesign | Cancel makes no job/provider call; confirmation admits one pinned Tier-1 task and returns a retained proposal/trial, without replacing the existing design or active prefix. |
| Accept a redesign, then Generate | Exact reviewed design revision is selected explicitly; deterministic compilation uses it with current approved content, and production activation remains separate. |
| Create a new prefix | Same-section Create Prefix tab yields a validated draft and human-accepted registered custom profile; it appears with Generate/Redesign and survives reseeding. |
| Redesign Prefix-Maker itself | Previous approved fresh generation reviews a quarantined successor; bootstrap and approval never depend on that unvalidated successor. |

Run this matrix through the production facade, composed ingress, Recipe worker and
browser path. Script-only success or a screenshot of buttons does not close the plan.
