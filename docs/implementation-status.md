# v3 implementation status and work ownership

Reconciled on **2026-10-23** against the common `main` working tree. Consolidation
started from `580a7f73e`; the raw-source checkpoint is `c52bdc2e0`. This index
also includes the subsequently validated pure-review prerequisites and independent
execution guard and per-step validator observation repair described below.
It describes the source delivered with this document.
It supersedes older present-tense implementation observations in the guides;
their binding component, authority and failure contracts remain authoritative.
The dated implementation journals retain historical evidence, including failed
attempts. Neither a journal entry nor this index establishes deployed support.

## Common source and validation

The authoritative source is the repository root and its single Git index on
`main`. The prefix and runtime workstreams are combined there. Former mutable
validation/build mirrors are retired after their exact inventories are checked
against retained evidence. Historical immutable sources, binaries and receipts
are evidence archives, not alternative development trees or Cargo build inputs.

On the current development host use only
`CARGO_TARGET_DIR=/Users/ollama/brassclaw-target`, with serial Cargo executions
from the repository root. `brassclaw-target2` is retired. Preserve relevant
executable evidence before cleanup and follow the disk rules in
[the development policy](development-policy.md) and [AGENTS.md](../AGENTS.md).
Other platforms use one appropriate local target directory; the macOS path is
not a product setting or a required installation path.

[The final reconciliation record](validation/2026-10-23-documentation-reconciliation.md)
maps these successor checkpoints and explains the canonical plan choices.

The consolidation validation result is recorded in
[the validation record](validation/2026-10-22-consolidation.md). The subsequent
[pure-review prerequisite record](validation/2026-10-23-pure-review-foundation.md)
covers 67 focused tests, actual PostgreSQL acknowledgement cuts, global child
execution, unchanged architecture rules and strict affected/root lint. Older frozen
receipts establish their named scopes only. No full release/platform matrix,
fresh remote installation, general crash recovery or full-plan completion is
inferred from focused local checks.

[Validator observation acceptance](validation/2026-10-23-validator-observation-limits.md)
additionally covers seven focused actual-worker/PG consumer tests, including
complete 300,000-item negative diagnostics under captured per-step limits after
later live reductions. Independent report bounds and changed-result rejection
remain enforced. This is not successful class Q1, pure approval or activation.

[Original-subject/input acceptance](validation/2026-10-23-validator-subject-inputs.md)
covers 29 focused contract, actual-worker/PG and ordinary-global cases. A distinct
sealed observer now verifies the original stored submission/validator selection
and every actual input edge under captured limits; identical outputs from changed
subject data cannot be relabelled as its observations. Shared borrowed fingerprints
preserve historical bytes and aggregate limits without cloning result trees.
Trusted Q1 writers, approved mandatory rules, interpreter/run/attempt/settlement
correlation and general pure activation remain open.

[Retained global-worker acceptance](validation/2026-10-23-retained-global-worker.md)
covers 16 fresh ordinary/PG/global and separate-workspace lifecycle cases.
Source-Inspection, catalogue preparation and ordinary global hosting now share
the existing private worker image, retained with unknown singleton ownership.
Actual start after removal of a temporary installation source, image lifetime,
policy/limits, activation/restart, child selection and cancellation/settlement
callers pass, with strict lint/format in both workspaces. This repairs live hosting
identity; durable interpreter/Q1 records and generic crash recovery remain open.

## Implemented boundaries

| Area | Actual implementation and supported boundary | Remaining qualification or cutover |
| --- | --- | --- |
| Global Monty | `global_monty_startup.rs`, `GlobalMontyOwner`, `GlobalMontyDriver`, `global_task_factory.rs` start one supervised Monty 1.0 worker before ordinary workers/producers. Opaque input IDs, isolated tasks, typed retained Recipe execution and reply/history use that service. The per-chat lazy driver is removed; there is no Rust-loop fallback. | Supervised durable recovery/replacement, unresolved effects, broader wait/child/restart and platform acceptance. A completed task does not stop the instance VM. |
| Components and preload | `component_revision`, `revision_store`, retained instruction/inputs/tools/reviews and `retained_preload`/`retained_source` implement immutable revisions, recursive contracts, typed bindings, dependency-first function loading and pinned exports for selected graphs. Runtime values are data. Pure-review source prerequisites now expose exact complete closure observations, all-export checks, strict declarations and original sealed subject/selection identities; [their contract](reborn/pure-component-review.md) distinguishes these from approval. | Migrate remaining legacy seeds/callers; general standalone pure approval and qualified dynamic flow/occurrences remain separate gaps. Pure evidence writers/readers, Human Q2, seed qualification and activation are unfinished. Legacy composer bodies are not the ordinary retained execution contract. |
| Approval and deployment | Trusted structural/behavioral evidence, human association review, coherent authored catalogue activation and component deployment exist. Exact packaged bootstrap qualification exists for named first-party closures. | Eight class-specific validator Recipes and their ordinary durable scheduling are incomplete. Existing narrow package qualification is not a general pure-component exemption. Authored changes require human Q2; shipped seeds require controlled integrity/Q1/behavior, without extra installed-instance Q2. |
| Routing and MCP | Ordinary matching uses a coherent approved generation. Qualified command discovery, always-running MCP server and ordinary new-chat ingress/response correlation exist. Packaged public reply has scoped ordinary/MCP acceptance. | Populate and qualify the rest of the public library and its commands; migrate remaining compatibility surfaces. Draft/status labels are not advertised contracts. Only actual No-Match enters Tier 2. |
| Complete file source | The existing registered `builtin.read_file` has a strict `raw_utf8_preserved` profile returning the complete captured UTF-8 text, host invocation/content identity and explicit representation/completeness. Existing live read limits, current Tool policy and mount/structured-entry protection apply. [Scoped native acceptance](validation/2026-10-23-raw-file-snapshot.md) covers actual local and PostgreSQL callers and backend races. | Canonical raw Skill/export/ToolSkill/Recipe qualification, independently approved R10 pure checking, retained source/derived lineage and doc-sync/doc-convert consumer migration remain open. A numbered preview or caller flag is not raw provenance. |
| History and native custody | Exact submitted input and eligible history helpers exist; native Tool/model answers are retained before later validation or failed acknowledgement and redundant host payloads are freed after the original durable acknowledgement. | Finish remaining compatibility consumers and generic recovery/operator reconciliation. A journal acknowledgement cannot replay an effect or turn its original failure into success. |
| Child tasks | Explicit `workflow:{recipe_id,variant}` plus typed `inputs` reaches native admission with the original parent claim, invocation and captured catalogue. The actual child runs on the same global VM, uses dependent admission, returns its full correlated reply or classified terminal outcome and retains old selection. Readable ensure/input/run acknowledgement loss is reconciled by observation, without resubmission. | R09 installation-owned parent qualification/activation, all five historical subagent Recipe migrations, saturation, unreadable/delayed outcomes and restart. Legacy flavor/task authorization receipts are not child completion. Closed development review does not activate its parent Recipe. |
| Atomic transcripts/admission | Shared submitted-input binding rejects rebinding/redaction/wrong kinds; exact acknowledgement is read-only. PostgreSQL admission binds the existing input in the same transaction as the run/aliases/reservation. V176 adds canonical scoped inbound receipts with atomic message acceptance and checked legacy backfill. Memory/filesystem callers keep exact event identity and reject changed duplicate content. | Filesystem pending/partial-write reconciliation, legacy unkeyed reservation release and generic recovery remain open. Ambiguous/corrupt old receipts fail migration rather than selecting a winner. |
| Resources and live settings | Revisioned settings API/WebUI and acknowledged Rust/Monty publication cover heap, task compute, transport/hosting/queue limits, coding limits and native accounting. Default task compute is 600 executing seconds, excluding idle/queue/external waits. Startup/manual shared heap has a 512 MiB minimum; startup sizing is the default, optional automatic monitoring is separate. | Complete remaining cap inventory, effective-limit/platform/browser acceptance and general recovery. Fixed transport depth 64 and snapshot conflict-attempt policies (PG 12, filesystem 8) are still open; do not claim every value is adjustable. |
| Token policy | Discretionary token budgets default off, with accounting retained and eligible retrieval/history/task consumers uncapped by those quotas on the migrated path. Provider/model technical context/output limits remain independent. | Audit unmigrated consumers. Per-provider/model prefix/prompt policy is stored with provider revisions but unconditional final-dispatch enforcement is still required. |
| Prefix workstream | Six profiles, immutable originals/generations/maps, retained model-artifact inspection/tokenization, source-only compiler jobs and measurements, provider/model limit storage/UI, budget planning and source-check primitives exist. | Named/composite target-aware Generate/serving, accepted designs, Sources jobs/projection, Redesign/Create, final request guards, current cache/quality and installed acceptance. Source-only/quarantined generation is not activation. See the prefix plan's current delivery table. |
| Sempai and shared facilities | Completed-turn delivery/effect/accounting custody and immutable proposal/deployment infrastructure exist. Shared PostgreSQL endpoint/TLS/pool construction already exists. | Recipe-owned Sempai evidence/proposal/iteration semantics and connection/lifetime consolidation belong to the existing v3.5 work IDs. Do not build another reviewer, provider registry, VM, connection builder or universal recovery ledger. |

The original seven composition failures were resolved on their recorded global
service path. That limited result does not establish that all plans or all
integration suites pass on this combined tree. The validation record identifies
which combined-tree checks have actually run.

**Independent pure-step support:** authored preparation now checks each variant's
unbound executable steps separately from usage-closure UUID coverage; retained
selection/MCP verification repeats that check. Only the exact existing packaged
generation workflow has independent coverage on this path. [Scoped acceptance](validation/2026-10-23-independent-pure-execution.md)
passed for the reader hardening, actual operator/PG/global publication, old
selections, restart, deployment and child callers. Validator Steps 3/8–10
still own general root/export/closure approval, durable pins and compatibility/repair cutover,
as detailed in [the pure review contract](reborn/pure-component-review.md#exact-remaining-cutover-contract).
Neither this gate nor the source/declaration prerequisites establish general
independent pure activation. Invalid historical dependency-only selections fail
explicitly; they are not silently grandfathered, removed or replayed via Tier 2.

## One implementation owner per responsibility

This table is a dependency/navigation index, not another implementation plan.
Implement the linked owner's steps once. Other plans consume its contracts and
acceptance; they must not maintain a second implementation of that responsibility.

| Responsibility | Canonical implementation owner | Consumers / shared gate |
| --- | --- | --- |
| Instance authority, live resources, global lifecycle/admission/cancellation/recovery and runtime cap migration | [simplified v3 remaining work](../simplified_v3.md#remaining-implementation--consolidated-2026-10-22), Phases 0, 1–3, 3a, 4–8 and §§8–10 | Validator, composition, prefix; v3.5 V35-01/23/24/26/27/29. Reuse the existing service and acknowledged settings owners. |
| Complete review subjects, eight Q1 routes, behavior/semantic evidence, human Q2 and general pure-component approval | [validator remaining work](../validator_v3.md#remaining-implementation--consolidated-2026-10-22), Steps 1–12 | simplified Phase 0a consumes it; composition does not implement another validator. Coordinate V35-03/04/17–19 with this owner. |
| Canonical reusable components, typed layouts, bindings/exports, library migration and Skill execution commands/MCP population | [composition remaining work](plans/monty.composition.md#remaining-implementation--consolidated-2026-10-22), §§6, 20–25 and 35 | [component implementation instructions](plans/monty.composition.implementation.md), coverage, Recipe and Skill-Recipe appendices. simplified consumes qualified library results. |
| Actual child admission/recovery infrastructure | simplified Phase 3a and its current remaining-work section | Composition R09 consumes it and owns parent/usage/Recipe qualification and the five seed migrations; no second scheduler or Rust task workflow. |
| Prefix source/profile/design/generation/model budgets, target serving and final model-request size guards | [prefix remaining sequence](plans/prefix_v3_upgrade.md#remaining-implementation-sequence--full-completion), §§4–12 | simplified §10 consumes canonical provider storage/guards; v3.5 V35-20/28 consumes prefix policies and measurements. No duplicate editable budget store. |
| Recipe-owned Sempai conversion, dynamic flow/occurrences and shared connection/facility consolidation | [path to v3.5](../path_to_v3.5.md#ongoing-plan-handoffs-not-parallel-implementations), V35-09–16/21–22/28 and its dependency table | [Sempai](plans/sempai_v3.5.md) and [connections](plans/simplify_connections_v3.5.md) retain source requirements; v3 plans reuse prerequisites without duplicating future conversion. |
| Combined upgrade, installed browser/provider tests, platform matrix and release | simplified Phases 7–8 and §5 | Each owning plan supplies its acceptance results. A commit/push is a checkpoint, not a full-plan/release certificate. |

## Resource settings: what remains fixed

The [runtime limit audit](runtime-limits-audit.md) and actual
`MontyExecutionLimits`, revisioned settings DTOs, validators and frontend help
are the detailed inventory. Existing WebUI explanations must accompany every
new operator-adjustable policy. Distinguish desired and effective values.

- The 512 MiB startup/manual minimum is the requested product floor. A ceiling
  is not reserved memory. Safe larger values are live; unsafe reductions fail
  or remain visibly pending according to the selected mode.
- The 600-second default is per-task executing time, not global lifetime or a
  wall-clock deadline for external work. Removed allocation-count enforcement
  is retained as retired configuration evidence, not silently treated as active.
- Transport depth 64 and the PG/filesystem optimistic conflict-attempt counts
  remain implementation policies to reconcile. Do not remove a parser guard
  before safe decoding/ownership/late-error cleanup and real deep-data checks.
  Conflict retries may repeat only pure snapshot transitions, never Tools.
- No active settings named `cachemax` or `max_shared_code_cache_bytes` were
  found in the audited runtime. Weak immutable code reuse is not a fixed
  task-data cache reservation. Keep auditing actual consumers rather than adding
  unused controls for absent names.
- Wire integer widths, checked sequence/version exhaustion, checksums and
  identity formats are representation constraints. Do not turn them into
  adjustable permissions or promise that an operator can exceed the protocol.

Every remaining implementation step must preserve exact old task selections,
live kernel policy, typed state, original effects and attempt fencing. Missing
support is implementation work; errors never become No-Match, empty success,
self-approval or a Rust/per-chat fallback.
