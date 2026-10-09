# Validator v3 implementation plan

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
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](skills.md#preloadable-function-interface-binding-v3-target).
This is a binding target, not proof of implemented loader/store/runner support.
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


Status: proposed implementation plan; no component activation or runtime acceptance.
Source audit: 2026-10-07; architecture/baseline reconciliation: 2026-10-09.
Monty documentation reconciliation: 2026-10-09; pinned upstream documentation
and local control extension examined. Section 7 gives concrete implementation
instructions and examples; it supplements every applicable step below.
Reinspect the working tree before each implementation slice. Source presence and
reported prior acceptance are not evidence that this checkout is deployed.

## 1. Goal, authority and boundaries

Implement Q1 as approved, versioned validator Recipes assembled from reusable
PythonCode and existing Tool usages. Monty owns validation sequencing. Rust
provides parsers, primitive operations, VM hosting, kernel enforcement, durable
evidence and transactional activation infrastructure. Do not replace the eight
Recipes with eight specialized Rust validator Tools or a Rust validation loop.

The binding ground truth is [recipe.md](recipe.md), [skills.md](skills.md),
[tools.md](tools.md), [toolskills.md](toolskills.md), and
[simplified_v3.md](simplified_v3.md), especially sections 1.1, Phase 0a, Phase 3a,
Phase 7, Phase 8 and the system-component override rules. Follow
[docs/development-policy.md](docs/development-policy.md) for development evidence.
If this plan conflicts with those guides, correct the plan before implementation.
Historical class labels, archive examples and source comments are not authority.
Coordinate implementation with [monty.composition.md](docs/plans/monty.composition.md),
especially sections 20, 23–26, and its
[per-Skill execution Recipe inventory](docs/plans/monty.composition.skill-recipes.md).
The composition plan delegates the eight validator successors to this plan;
coordinate shared loader/function qualification rather than commissioning a second loader
or duplicating canonical usages. Resolve proposed roles to actual retained UUIDs.
Use [the pinned Monty documentation manifest](scripts/prefix/monty-reference/v1.0.0/manifest.json)
and [the local control extension contract](vendor/monty-control/BRASSCLAW.md)
for interpreter behavior. Online examples explain concepts but do not establish
support in this checkout. Section 7 records the important differences.

Completion means all eight class routes validate actual proposed immutable
components and relevant combinations through the real admission/IBS/composition/
Monty/host path, retain trustworthy evidence, and permit coherent activation only
after the applicable provenance gates. Installed authored changes require human
Q2; bundled installation/upgrade components use controlled development review,
integrity/Q1 and behavioral qualification without additional installed-instance
human Q2. A validator verdict is not activation,
combination approval, Tool permission, semantic proof or whole-workflow completion.

No ordinary runtime cutover is authorized by completing a helper or writing a
Recipe. Complete the shared simplified-v3 prerequisites and production acceptance
before advertising this subsystem as shipped. Classes 10/50 have separate
protected-root requirements; they must not be accidentally certified by a generic
class-1/2/3 route. Their coordinated review is an explicit dependency below.

## 2. Verified baseline and affected consumers

| Finding | Actual source | Consequence and required disposition |
| --- | --- | --- |
| Eight Recipes share a three-field nonempty check | `builtin_bootstrap.rs`, `PC_VALIDATOR_STRUCTURAL_BODY`, `validator_recipe_row`, `seed_validator_recipes` | Replace with class-specific workflows using reusable checking components. |
| Exact seed source contains an unresolved marker in a comment | Same body; `q1_orchestrator.rs::run_validator_python` rejects remaining `{{vars.` | The checked-in seeded body fails the compatibility conversion; test exact seed content, not the shortened copied test body. |
| Candidate projection exposes only name/description/content | `q1_orchestrator.rs::reviewed_content_fields` | Supply the full immutable candidate and required structured dependency/implementation facts. Recipe class 21 currently receives its description as content. |
| Q1 extracts one body instead of composing a workflow | `q1_orchestrator.rs::load_validator_program` | Replace single-step extraction with retained Recipe/variant selection and IBS/composition. Preserve rejection of malformed or ambiguous selection. |
| Validator lookup uses exact legacy scope and mutable validated labels | Same lookup | Select from one coherent instance validator catalogue with verified provenance, not cross-scope SQL fallbacks or status labels. |
| No normal application caller of the legacy Q1 runner was found | Repository call-site search; `serve.rs` only comments on the retired sweep | Wire durable submitted review work into the global orchestrator. Do not restore an automatic Rust sweep executor. |
| Candidate seals exist but do not establish v3 approval | `validation_review.rs`, `validation_queue.rs`, V097 | Reuse exact-change protection; migrate to immutable review identities and retain validator/dependency manifests too. |
| Immutable draft revisions already exist | `component_revision.rs`, `revision_store.rs`, V101 | Extend/reuse this storage. Do not build a competing revision store or treat retained drafts as active. |
| Immutable review submissions and transactional staging exist | `review_submission_store.rs::submit_in_transaction`, `pg_review_submission.rs`, product authoring facade | Reuse exact candidate/base/dependency references and idempotent submission identities. Complete missing producer/admission connections; do not create another submission store or claim existing submission implies Q1 scheduling. |
| Ordinary startup already owns the shared global service | `global_monty_startup.rs::GlobalMontyOwner::start`, `runtime.rs`, installed catalogue | Reuse this service and task transport. Remaining work is validator admission, preload/interface support, full catalogue activation and associated acceptance; do not rebuild the global lifecycle. Packaged reply/history support is not full-library support. |
| Source inspection still enforces the earlier body contract | `retained_source.rs`: `result_store_sites`, `direct_host_calls` | Extend inspection for effect-free definitions, returned values and reachable helper calls together with retained export invocation. Do not merely remove one-call/result checks and thereby admit uninspected effects. |
| Association/schema parsers and evidence readers exist | `association_contract.rs`, `value_contract.rs`, `association_review_store.rs`, V102 | Reuse structured formats; connect actual trusted producers and activation. Parsing declarations is not approval. |
| New structural/behavior producers exist separately | `pg_retained_q1.rs`, `retained_source.rs`, retained executor | Reuse observations and persistence; do not claim these execute the eight Recipes or establish human/semantic approval. |
| Working-tree human association review additions exist | `pg_association_review.rs`, `association_review_store.rs`, product facade and WebUI association-review panel | Reuse exact displayed-view checksums and transactional Q2/association writes after verifying their real ingress and tests. Their declared scope is one Tool usage; `catalogue_activated` remains false. They do not establish eight-class review or production activation. |
| Legacy seed review requests now stay pending and persistence errors fail boot | `builtin_bootstrap.rs::record_seed_review`, `zencoder_bootstrap.rs`, `ValidationQueueStore::retain_pending_request`, `boot_integrity.rs` (2026-10-08) | Fabricated Q1/builtin graduation was removed. The controlled immutable system-seed evidence producer/reader is still required; pending requests and historical validated labels establish no v3 approval. |

The principal integration files are:

- Component stores/proposals: `crates/brassclaw_reborn_composition/src/pg_*_store.rs`,
  `sempai_proposal_sink.rs`, `pg_component_db_backend.rs`, `docplan_dissector.rs`.
- Review: `q1_orchestrator.rs`, `validation_queue.rs`, `validation_review.rs`,
  `pg_retained_q1.rs`, `pg_review_submission.rs`,
  `crates/brassclaw_skills/src/{association_review_store,review_submission_store}.rs`.
- Assembly/execution: `crates/brassclaw_engine/src/memory/{instruction_builder,
  composition,retained_instruction,retained_inputs,retained_tools,retained_reviews}.rs`,
  `crates/brassclaw_engine/src/executor/{retained_source,retained_recipe}.rs`,
  `crates/brassclaw_reborn_composition/src/global_recipe_ports.rs`.
  Include the selected Monty source-inspection utility, retained interface readers,
  invocation transport/result capture and continuation records when extending
  their contracts; locate actual owners before introducing new modules.
- Lifecycle: `component_boot.rs`, `content_integrity.rs`, `boot_integrity.rs`,
  `runtime.rs`, global Monty ownership/admission/driver modules, product turn runner.
- Review UI/API: `pg_recipe_store.rs`, `webui.rs`,
  `crates/brassclaw_product_workflow/src/{recipes,reborn_services}.rs`, WebUI
  component/validation handlers and static component-detail/validation-queue pages.
- Downstream publication: intent inputs, retrieval/matching, component caches,
  Kohai/Sempai prefix generation, seed upgrade/repair and backup/restore.

These paths are inspection targets, not a directive to edit every file. Identify
actual callers, interfaces, feature gates and crate-local rules before editing.

## 3. Target review flow and invariants

```text
Installed authored change: Author/WebUI/Sempai proposal
  -> immutable draft revision + exact proposed dependency selection
  -> durable review submission
  -> select already-approved validator Recipe/variant and complete closure
     from one consistent catalogue generation
  -> IBS BuildInstruction -> retained interface/export selection
  -> effect-free dependency-first preload in the existing global service
  -> typed invocation inputs and matching ToolSkill binding preparation
  -> global Monty invokes pinned Q1 functions/pure steps over candidate data
  -> trusted mechanism retains verdict, observations and exact review manifest
  -> constrained behavioral validation of the proposed usage/workflow
  -> authenticated human Q2 reviews the exact evidence set and combination
  -> trusted approval writer + coherent catalogue activation
  -> future tasks select the new generation; existing tasks keep their selection

Bundled installation/upgrade package
  -> installation owner verifies exact package/source/artifact provenance
  -> noncircular structural/Q1 and observed behavioral qualification
  -> trusted system_seed evidence/combination writer (q2_ref = null)
  -> coherent publication after loader/runner/integrity acceptance
  -> preserve overrides and retained old selections; no instance human Q2
```

The second path applies to shipped validators and the global root as well as other
first-party components. An installed edit cannot select that path through a source
label. Bootstrap qualification belongs to the installation owner, not to a validator
approving itself or a request for external maintainer approval.

Q1 and behavioral evidence may have separate records. Authored Q2 must cover the
required successful records for the exact candidate/combination; system_seed
qualification must retain its required integrity/Q1/behavior records instead.
A negative behavior case
can pass because the expected failure was observed; this does not mean the Tool
or task succeeded. An incomplete, cancelled or crashed review has no passing
verdict. Runtime/DB/provider errors are distinct from candidate defects.

Mandatory invariants:

1. Candidate bytes, schemas and proposed code are data during inspection.
   Candidate source is executed only in the explicit draft-testing environment.
2. Validator Recipes and executable dependencies are already approved. The
   candidate being inspected may be a draft; requiring its own Q1/Q2 beforehand
   would create a circular gate. Ordinary matching never sees that draft.
3. One UUID per component step. Pure checks need no ToolSkill. Each Tool usage
   has one Rust binding step immediately followed by its class-22 execution step
   and a complete Skill/code/ToolSkill/Tool association. Independent calls stay
   separate; supported direct dependent chains must cover every binding.
4. Use persisted `knowledge`, `stepnumber`, `type`, `include`, variants and
   `step_link`. Do not create `type: llm`, execute `text`, or use diagram-only
   `channel`/`step_id` as insert fields. Recipes store stable UUIDs without versions.
5. Runtime inputs use step-local `inputs["local_name"]`. References in binding
   metadata follow recipe.md's exact whole-value grammar. No source substitution.
6. One retained task snapshot pins validator Recipe/variant/layout, complete
   interface/code/context/Tool graph, export-to-implementation resolution,
   deterministic load order, implementation artifacts and approval references.
   The proposed candidate graph is separately identified as the review subject.
7. Preserve exact schemas, missing/null/default rules, computed arguments,
   effects, outcomes and retry counts. No invalid-output repair or effect replay.
8. Kernel policy/settings remain live before every dispatch, including validation
   and retries. Review/activation does not create scoped grants or invocation leases.
9. Authored validator updates need Q1, behavior, human Q2 and required combination
   approval. A successor is reviewed by an existing approved validator; it cannot
   choose itself or self-issue approval. First bootstrap uses the controlled seed
   contract, not a recursive request for a nonexistent validator.
10. Errors in active catalogue integrity are explicit repairable errors. Drafts
    remain excluded before matching. Do not silently hide a broken active workflow
    or route it through Tier 2. Only actual No-Match permits Tier 2.
11. Preload installs only qualified definitions/immutable constants and causes no
    usage effect. Invocations use local parameters and return values; result capture
    belongs to the invocation boundary. Loading is not a ToolSkill binding or a
    separate effectful Recipe step. Mutable defaults, closures, arrays and host
    contexts must not leak across tasks, attempts, children or invocations.

## 4. Implementation sequence

Each numbered step has a deliverable and acceptance gate. Later steps may be
prepared as drafts, but must not activate against incomplete prerequisites.

Dependency order is explicit: steps 1–3 define the review subject and trust
contracts; step 5a qualifies interface loading/invocation before steps 6–8 can
qualify replacement workflows in the constrained review environment; step 9
connects durable review and human decisions. Step 10
prepares publication but cannot publish the first new validator generation until
step 11 establishes its noncircular bootstrap trust and step 12 verifies the
coordinated production startup/admission path. Implement those prerequisites
before exercising publication, then run the final end-to-end activation case.
An existing validator may review successors only after its own provenance and
required evidence are verified; the legacy `validated` label is insufficient.

### Step 1 — Freeze contracts, inventory primitive reuse and define impact

1. Re-read the five guides, development policy and applicable subsystem rules.
   Record the commit and relevant working-tree changes; preserve operator edits.
   Reconcile composition sections 20 and 23–26 and the canonical Skill Recipe
   inventory. Record reuse/partial overlap/blocked dispositions for each usage;
   a preparation dictionary or pure helper must not become an artificial Skill.
2. Enumerate the eight actual Recipe/PythonCode UUIDs in the supported store,
   their bodies/checksums, variants, scope keys and current users. A read-only
   live inventory is required before any migration; checked-in names are not UUIDs.
3. Trace every proposal/save/resubmit/approve/reject/bootstrap/repair path and
   every reader of validated status, queue state, confidence, tier or consumer tags.
   Include class-10/50 protected updates and non-Recipe classes in the UI facade.
4. Inventory existing primitives for exact catalogue reads, parsing/source
   inspection, schema checking, draft execution, evidence persistence and model
   calls. Compare registered callable/adapter contracts, not names or seed prose.
5. For each operation, record reuse, missing infrastructure or a genuinely missing
   primitive. A new primitive must expose a bounded general operation with a
   concrete contract; never expose `validate_class_N` or a whole Q1 workflow Tool.
6. Specify input/result schemas for the proposed review context and each reusable
   check. Use strict parsers at the raw-byte boundary so duplicate JSON keys are
   rejected before conversion to a generic JSON value loses that information.
7. Fix review scope, required class checks, evidence sufficiency and activation
   ownership in a decision record. Define a private, typed review admission path
   using the existing global service transport, without fabricated chat input.

Gate: a source-backed mapping of requirements to consumers and reusable primitives
exists; missing APIs are labelled implementation work. No symbolic JSON is inserted.

### Step 2 — Establish immutable submissions and complete review subjects

1. Reuse `ComponentRevisionDraft` and `PgComponentRevisionStore`; extend the
   class-specific revision documents only where the actual retained consumers
   require missing execution-relevant fields. Define strict allowed fields first.
2. Reuse `PgReviewSubmissionStore` and the operator facade for a submission pointing
   to the exact candidate revision/checksum, base revision if replacing, and proposed
   dependency graph. Do not project away
   schemas, bindings, includes, variant layout, effects or implementation identity.
3. Use `submit_in_transaction` to stage the draft and submission atomically through
   the authoring owner. Preserve existing compare-and-swap allocation and exact-ID
   retry semantics; test extensions without duplicating already supported storage.
   Work admission/outbox linkage remains a separate integration gate in step 9.
4. Migrate WebUI/Sempai/component-management writers to that path. Upgrades create
   new revisions; they must not overwrite the live approved row during Q2.
5. Map legacy proposals without inventing approval. Preserve exact originals and
   proposed values. Missing Skill associations/domain overviews become explicit
   migration work, not fabricated associations or silently reclassified approvals.
6. Define how a review chooses proposed draft dependencies versus already-approved
   dependencies. Never substitute latest or downgrade during the review. A new
   dependency combination needs combination evidence even if schemas are compatible.
7. Define and implement a strict retained Skill-interface declaration within the
   checksummed Skill revision document: exported names/signatures, associated code
   UUID/implementation symbols, private helpers/constants, typed contracts,
   dependencies and fixed/computed arguments. Validate identifiers/reserved names,
   explicit keyword parameters or a declared inputs mapping, and compatible entry
   forms of the same one-Tool usage. Reject untyped variadic dispatch. Keep both
   skill-association formats unchanged; do not append interface fields to them.
   Add supported storage/reader/authoring contracts before accepting these records;
   existing generic document storage alone does not implement their semantics.

Gate: edits/resubmissions/concurrent proposals cannot alter an existing review
subject; legacy live components remain available during staged migration. Raw
candidate data can contain quotes, braces, Unicode and Python-looking text safely.

### Step 3 — Retain validator selection and trusted evidence contracts

1. Replace exact-scope mutable lookup with one instance catalogue owner selecting
   an approved active validator route for the requested class. Keep class routing
   distinct from ordinary user intents; retain the `05:validator` internal route.
2. Select the Recipe revision, embedded variant, `step_link`, layout and full
   transitive graph in one repeatable-read/serializable catalogue view. Reuse the
   revision/evidence transaction interfaces; do not stitch separate latest reads.
   Include the checksummed interface, selected export/implementation symbol,
   qualified helper imports, dependency order and canonical execution Recipe
   identity in selection and evidence. Multiple exports require a supported retained
   invocation selector; never infer one from prose/order/name or invent persisted
   Recipe fields. Trace snapshot, transport and continuation readers together.
3. Retain actual Tool artifacts/adapter identities for the validator's usages.
   `RetainedToolBinding` deliberately does not contain the executable artifact;
   implement and verify that missing resolver/retention boundary before dispatch.
   Verify old artifacts are actually reloadable through the supported ABI/registration
   path after upgrade/recovery. A captured image/checksum is insufficient. Block
   an upgrade that would strand retained work; never substitute a latest handler.
4. Define durable review records linking the subject graph to the exact validator
   manifest, rule coverage, actual observations, verdict and attempt identity.
   Reuse V102 storage where its evidence contract fits; add linked records or new
   migrations where it does not. Do not alter exact skill-association formats.
5. Keep existing legacy seals as migration protection. A stale review of a new
   proposal must not pass because an old mutable row still compares equal.
6. Evidence persistence accepts opaque executor/review-owner outcomes, not public
   caller-supplied `passed`, `succeeded`, provenance or successful observation JSON.
   Preserve original evidence IDs/bytes on uncertain commits and retry persistence
   idempotently, without rerunning completed Tool effects.

Gate: missing/ambiguous validators and missing evidence fail explicitly; a validator
upgrade during review does not change that review or erase its selected code.

### Step 4 — Repair the shared seed defect without mutating approved originals

1. Make the targeted compatibility regression use the exact checked-in seed body.
   Verify the comment-marker rejection through the actual Q1/Monty caller.
2. Author replacement code with direct `inputs[...]` access and no legacy slot
   preparation. The final path must not rewrite approved source at all.
3. While the legacy path remains, isolate its compatibility handling and tests.
   Candidate values and inert marker-looking text must remain data. Do not make a
   broad string scanner the final substitute for parsed source/binding validation.
4. Stage the replacement with supported revision/upgrade contracts. Fixing a Rust
   constant does not update existing DB bodies under `ON CONFLICT DO NOTHING`.
   Do not perform blanket repair or mutate original bodies/checksums.
5. The minimal fix can restore only the old structural check. Do not activate it
   as a complete final-v3 validator or upgrade its confidence as a shortcut.

Gate: the exact replacement body runs with unchanged source/checksum and hostile
input data; the original remains retained and its limitations are visible.

### Step 5 — Implement reusable facts and checking components

1. Reuse `value_contract.rs`, `association_contract.rs`, retained graph readers
   and source-inspection observations. Fix any spec discrepancies there with
   focused consumer tests rather than introducing another schema framework.
2. Separate generic facts from editable validation policy. Parsing, checksum/
   artifact verification and registration inspection may be Rust primitives.
   Recipes/PythonCode select class requirements, combine diagnostics and decide
   the validation verdict. Kernel/adapter safety constraints cannot be disabled
   by editing a Recipe.
3. Author small PythonCode components for identity/metadata, recursive contracts,
   graph completeness, argument compatibility, workflow structure, source facts,
   retry declarations and verdict aggregation. Reuse them across class routes.
4. Every component declares its input/result contract and evidence limits.
   Diagnostics identify the failing rule and field/step path. Bounds reject the
   entire review rather than silently truncate checks, dependencies or reports.
5. Source inspection parses the candidate as text. Do not execute candidate code
   to discover its call sites. Distinguish static facts from guarantees: one
   return or boundary `result` assignment does not prove all paths satisfy outputs.
   Extend parser observations and trusted readers for definitions, signatures,
   resolved function references, reachable helper calls and load-time expressions.
   Match transitive Tool calls to the owning usage/bindings; reject unresolved
   dynamic calls under the unsupported profile. Do not just relax the current
   `result_store_sites`/direct-call restriction. Function definitions use `return`;
   the qualified invocation boundary captures their returned result.
6. Internal includes require real recursive resolution/assembly and checksum
   pinning. The current retained Tool program rejects nested code; extend its
   supported assembler and tests before approving validators that depend on it.
7. Likewise, implement permitted dependent-chain binding layouts explicitly.
   Until supported, those drafts cannot activate; do not redefine permitted v3
   chains as authoring defects or silently exclude an already-active workflow.
8. Apply section 7.3's bounded data conversion and 7.4's source/profile rules.
   Reuse the existing strict Monty-to-JSON adapter rather than adding a lossy
   converter. Optional static typing cannot replace runtime contracts or Q1.

Gate: shared checks have positive/boundary/negative cases, parser facts are not
labelled semantic approval, and no class-specific workflow is hidden in a Tool.

### Step 5a — Qualify effect-free preload and pinned function invocation

This is a prerequisite for steps 6–8 and new Skill activation. Implement it in
the existing composition/interface/host owners, coordinated with composition
section 24. It is not another validation workflow or a new global VM.

1. Resolve declared code/helper/constant dependencies by stable UUID from the
   selected closure. Reject undeclared dependencies, cycles, missing symbols,
   incompatible signatures/imports and export conflicts before any effect.
   Compute dependency-first topological order, breaking ties by stable UUID.
2. Qualify exact definitions under the selected packaged Monty build. Reject
   Tool/model/I/O/scheduling calls during load, including default expressions,
   decorators and initializers. Only supported definitions and recursively
   immutable constants qualify; uppercase list/dict/set names are still mutable.
3. Implement a collision-safe task-local export environment binding each public
   name to its owning Skill/interface/code revisions, implementation symbol and
   invocation binding context. Do not use runtime-generated eval/getattr/source
   to resolve names. Never capture ambient or another task's host authority.
4. Share compiled artifacts only with compatible symbol environments. Allocate
   fresh mutable arguments/defaults/working objects per invocation; forbid shared
   mutable defaults/globals/caches or closures over task values. Where safe sharing
   is unavailable, use isolated function environments within the one global service.
5. Invoke the selected export with validated typed data after the matching binding.
   Capture its return into the step result and validate the output separately.
   Preserve fixed selectors and computed argument guards; no source reconstruction
   or effectful load step is introduced. Independent invocations retain separate
   Recipe occurrences, counts and effect identities even if code is deduplicated.
6. Pin export environments across children, waits, retries and resumes. New tasks
   may select a successor without overwriting old names/bindings. Extend actual
   continuation/transport contracts and retain old code/native artifacts until
   their references expire; do not use a new feed's latest name resolution.
7. Follow section 7.2's suspendable invocation design. The pinned
   `MontyRepl::call_function` is not the production Tool-usage entry point: it
   accepts positional arguments only and cannot service external/OS calls.
   Do not copy upstream pool/check-out examples into the global lifecycle.

Gate: real retained Monty execution demonstrates zero load effects, deterministic
resolution, signature/cycle/collision rejection, fresh repeated/concurrent state,
live policy after preload and exact old/new export/host coexistence through waits
and supported recovery. Parser acceptance alone cannot qualify this loader.

### Step 6 — Assemble and execute multi-step validation Recipes

1. Replace `load_validator_program` body extraction with retained internal Recipe
   selection, real `step_link` compilation, ordered IBS and typed composition.
   Use the named internal selection path; do not invent a user intent to validate
   components or require ordinary chat examples for this private entry point.
2. Extend/reuse `GlobalRecipePorts`, retained execution and task ports so the
   global Monty orchestrator sequences each check. Rust executes only requested
   primitive operations/steps and transports observations; it never chooses the
   next validation step or runs a second Recipe loop.
3. Supply candidate/context/fact values as declared local inputs. Retain outputs
   by stable step identity and explicitly bind downstream consumers. Enforce
   recursive producer/consumer compatibility and concrete output validation.
   Use step-5a preloaded functions for Tool usages. Preserve the distinction
   between source definitions, retained export invocation and result capture.
4. Install actual approved Tool bindings before matching PythonCode executes.
   Do not execute ToolSkill text or treat a returned directive as proof of loading.
5. Add explicit model steps through supported model usages where a validation
   Recipe requires reasoning. A failed model call is an incomplete review, not
   a pass or No-Match. Deterministic validators make zero LLM calls.
6. Retain review task identity/cancellation and runtime compute/memory accounts.
   External waits yield so other tasks/control events proceed. The global service
   stays alive after validation completes or fails; no per-review orchestrator VM.
7. Preserve classification between candidate failures, incomplete evidence and
   infrastructure errors. No malformed `{pass, errors}` result may advance Q1.
   Host denial/cancellation/stale attempts, exhausted budgets and recorded unknown
   effects remain authoritative even if candidate Python catches an exception
   and returns a success-shaped value.
8. Define a supported retained control/occurrence contract for stop-on-failure,
   qualified conditional semantic/model checks, repeated checks and waits. Inspect
   the current compiler/root/task ports first; extend their owning contracts where
   missing. Do not flatten conditional effects into a straight step list or add
   invented branch fields to persisted Recipe JSON. Pin selected layouts and track
   executed occurrences/continuation positions in trusted review state.
9. Test each branch's required coverage and terminal verdict. Deterministic routes
   never invoke the model; explicit reasoning routes retain their tier and exact
   context. An interrupted, missing or skipped required check remains incomplete.
10. Apply section 7.6's host-call correlation and 7.7's resource/error handling.
    Upstream feed clocks, exception names and pool defaults do not implement the
    durable BrassClaw task contract. Reuse existing task accounts/interruption.

Gate: a real multi-step validator transports typed results across an external
wait and interleaved task, preserves its snapshot, and cannot replay a completed
check's external effect after output/evidence persistence failure. Qualified
branches stop or continue exactly as declared without fabricating coverage.

### Step 7 — Author all eight class-specific validator Recipes

Keep stable class routing, reuse common checks, resolve real component UUIDs,
and apply the following requirements. The names below identify existing routes;
they are not new Tool names or new constructor fields.

| Route | Required ordered checks and verification |
| --- | --- |
| `validator-class-0` / Tool | Identity and primitive grain; explicit operation/effect contract; registered schema versus Rust/host adapter; recursive inputs/results and transport limits; capability/callable/Tool UUID/alias mapping; retained artifact/checksum/ABI availability; actual errors/waits/cancellation; read-only/deduplication claims and constraints. Verification loads draft artifacts only in the constrained environment, measures declared behavior, and checks live policy across aliases/old versions. Compilation alone never passes the required review. |
| `validator-class-1` / Skill | Prose sections and one-usage meaning; strict checksummed interface with public names/signatures, associated UUID/implementation symbols, private dependencies and compatible entry forms; effect-free preload including defaults/initializers; dependency-first symbol resolution, immutable constants and isolated mutable state; pinned old/new export/host coexistence; exact `skill-association/1`; class-correct UUID relationships; direct/computed/fixed arguments and actual callable compatibility; recursive return/error contract and exact failure metadata; transitive code/call graph. Verify the canonical execution Recipe/variant's stable identity, unambiguous command, captures, typed inputs, selected export, binding adjacency and completion/result contract. Reuse equivalent existing variants. Automated semantic and behavior checks compare purpose/defaults/computation/effects to prose; apply authored human Q2 or controlled shipped-seed qualification as appropriate. Private/protected usages remain unadvertised. |
| `validator-class-2` / Skill | The same complete usage checks as class 1. Correct the seed's obsolete domain-Skill label. Find multi-Tool domain overviews and migrate them with explicit reference updates into ExtensionCatalogue/Recipe structure; do not reinterpret class 2 as a hierarchy or mass-relabel existing rows. |
| `validator-class-3` / Skill | The same complete usage checks as class 1, plus verified explicit model-usage contracts where relevant. Class 3 alone neither executes an LLM nor makes a usage deterministic. Check actual model invocation, result/errors and consuming tier, without executing prose. |
| `validator-class-13` / ToolSkill | One non-executing/non-authorizing binding; Tool UUID/capability/callable/adapter/artifact agreement; registered parameter schema versus binding/usage contracts; fixed selectors and computed arguments; missing/default/null rules; recursive result/error expectations and numeric bounds; metadata include references/cycles/conflicts. Reject workflow/permission/retry execution in metadata. Test actual paired binding/export invocation availability; preloading a function must not dispatch or grant permission. |
| `validator-class-21` / Recipe | Parse actual persisted steps, variants and selected links; ordinals/classes/one UUID cardinality; matching binding/export invocation pairs; typed capture/local-input mappings; complete interface/dependency/code graph; supported export selection and effect-free assembly; result edges including optional/null/list guards; qualified branches/occurrences/waits; internal composition/call grain; tier restrictions; explicit retry/reconciliation/cancellation/final reply. Test each variant's routing, effects/history/completion and required negative paths. Canonical Skill execution variants must preserve exact command/capture/signature/fixed-argument/result agreement. Eligible mcp-call-skill-recipes declare exact command sentences, variable positions/types, escaping and valid examples; discovery projects approved available Recipes, never raw Skills. Internal validator routes have a typed private entry point and remain excluded from public matching/MCP. |
| `validator-class-22` / PythonCode | Parse exact source under the selected Monty profile; distinguish preloadable definitions from invocation boundaries and pure steps. Functions use explicit signatures/local parameters and return values; the boundary captures and validates the result. Inspect default/initializer/decorator expressions for load effects and resolve every reachable helper/call against the declared pinned graph. Reject undeclared/dynamic targets under unsupported profiles, shared mutable defaults/globals and captured foreign task/host context. Count transitive Tool calls against associations/bindings, including calls hidden in helpers; zero calls for pure logic, independent calls in separate occurrences and only supported covered dependent chains. Check recursive imports/includes/order/symbol contracts and actual returned/failure behavior. No data-to-source substitution or forbidden intrinsics. A pure helper needs no artificial Skill/ToolSkill; Tool usages require actual associations. |
| `validator-class-23` / ExtensionCatalogue | Domain overview plus actual task-group/Recipe inventory; correct target classes and reference resolution; child IDs, dependency closure and intent/index consistency; no hidden executable workflow or replacement for usage contracts. Validate schema supported by current store/consumers and real retrieval/navigation, not invented catalogue fields. |

For all routes reject unknown/duplicate structured fields, nil/wrong-class UUIDs,
cycles, incompatible arguments/results, invalid defaults, bool-as-integer values,
unrepresentable numbers and unjustified retries where applicable. Keep shared
schema checks deterministic. Semantic checks must report scope/limitations rather
than pretend arbitrary Python correctness follows from parsing.

Gate: every route has distinct class-relevant negative fixtures; nonempty invalid
components no longer pass merely because their descriptions are present.

Resolve the canonical execution variants for validator-owned Tool usages against
the composition Skill Recipe inventory before allocating new identities. A Skill
and its execution Recipe may need coordinated draft qualification; neither draft
is admitted to ordinary matching to break that authoring dependency. Keep private
review execution separate from MCP's completed-command ordinary-chat transport.
Keep canonical-Recipe review links distinct from executable dependency edges;
linking a Skill to its Recipe must not create a recursive code-loading cycle.
Do not require full MCP serving to activate an internal validator; advertise an
eligible public usage only after its own Recipe/discovery/chat gates pass. Routine
Skill invocations never create components or rerun Q1/behavior/Q2.

### Step 8 — Complete behavioral and semantic evidence

1. Reuse `pg_retained_q1.rs` actual structural observations and
   `BehavioralReviewSet` executor-owned data. Bind each record to the exact
   reviewed closure and validator/rule coverage; do not count the same observation
   as proof of a different graph or full Recipe completion.
2. Extend constrained review execution for pure logic, internal code and whole
   Recipes. Existing behavior producers cover selected Tool usages only. Verify
   outputs, effect counts, waits, reply/history/completion and negative outcomes
   at the appropriate level, without ordinary matching exposure of drafts.
   Include separate preload observations (zero effects), invocation observations
   (returned values/transitive dispatches) and branch coverage. A successful load
   cannot count as a successful usage; a valid canonical command must exercise its
   actual selected export/binding/result in constrained draft execution. Repeat
   ordinary-chat matching acceptance after qualified publication; never expose
   drafts to normal routing to obtain that evidence.
3. Let Recipes define representative expectations/cases through approved contracts.
   Trusted execution measures the actual observations. Required case coverage
   cannot be omitted by a candidate submitting an empty test list.
4. Define reusable semantic-review usages for prose/code/binding agreement where
   model reasoning helps. Retain the reviewer/model/prompt components and exact
   output as evidence. LLM verdicts are fallible review evidence, not proofs or
   substitutes for mandatory behavior, authored human Q2 or controlled shipped
   bootstrap qualification.
5. Move changeable audit prompts/check sequencing from the orphaned
   `executor/code_audit.rs` workflow into approved review components where it
   supplies useful checks. Preserve hard protection against self-modification
   in the trusted boundary. Resolve class-10/50 root/scaffold review and supervisor
   reconciliation separately; generic Skill approval must never replace it.
6. Run section 7.10's class-specific fixtures and 7.11's interpreter-boundary
   cases through the selected packaged build. Distinguish candidate source
   inspection, contained behavioral execution and production-path acceptance.

Gate: an inclusive/exclusive interval mismatch with otherwise identical schemas
is caught by actual behavior/review; malformed source, denied policy, unknown
completion and failed downstream output each retain honest, non-replayed outcomes.

### Step 9 — Wire durable Q1 work, human Q2 and approval writers

1. Reuse existing immutable submission records and make their review work discoverable
   via the existing admission/coordination services. Atomically retain submission
   and required work/outbox reference, or specify its idempotent reconciliation
   transaction before coding.
   Do not create a second durable turn scheduler or polling LLM loop.
2. Connect submission/resubmission and recovery to global Monty admission. Keep
   review claims/freshness distinct from Tool permission. Record the exact review
   manifest before the first effect; retain counts and confirmed/unresolved effects.
3. Persist a Q1 pass only after required check completion and exact subject/
   manifest validation. Database failure preserves pending/incomplete state.
   Rejections retain diagnostics; retryable infrastructure conditions do not
   falsely label a valid candidate defective or enable human approval.
4. Reuse and verify the working-tree human association-review route, exact-view
   checksum and transactional writer. Trace its instance-operator mounting and
   rejection through model/component-management ports. Extend the supported review
   owner to all required component classes and evidence sets; its present one-Tool
   usage scope is insufficient for whole Recipes and pure logic. A literal `human`
   field is not proof of authenticated human action.
5. Human review verifies semantics, coverage and the selected Q1/behavior records.
   Write the immutable human evidence and exact association approvals through
   trusted writers using skills.md's formats unchanged. Required reused dependencies
   can retain individual approvals; the new combination gets its own evidence.
   This human ingress applies to installed authored changes. Shipped packages
   instead use step 11's controlled installation-owned evidence writer; do not
   manufacture human records or require extra external/instance approval for them.
6. Define workflow approval/evidence for Recipe and pure-logic roots without
   pretending they own a `skill-association/1`. Extend linked review storage and
   readers explicitly where V102 usage-specific contracts do not suffice.
7. Remove legacy approve/update fallbacks only after their callers use the new
   contract. Audit all class paths, not only the Recipe facade. A missing, failed,
   stale, partial or swapped review set must prevent graduation atomically.

Gate: authored changes cannot activate through status/source labels, direct
component writes, a fake human marker, old receipts or client-supplied evidence.

### Step 10 — Prepare coherent activation and update downstream readers

1. Define the active catalogue-generation transaction linking approved immutable
   revisions, class-validator routes, required approval records, implementation
   availability and compatible intents/variants. Enforce concurrency revisions.
2. Prevalidate every consuming affected combination before publication. A changed
   shared validator/helper/binding can affect many routes/usages. Keep incompatible
   combinations in draft; do not silently switch dependencies of approved roots.
3. Prepare required prefix/cache artifacts, then atomically publish the generation
   or use a documented pending/effective publication protocol if artifact work
   cannot share the SQL transaction. Until publication succeeds, the old generation
   remains active. Never claim atomic DB-plus-external-service execution.
   First production publication is blocked on steps 11–12 prerequisites and the
   applicable acceptance cases; preparing this transaction is not authorization
   to graduate incompletely reviewed validators.
4. Update matching, named lookup, retrieval, IBS, prefix compilation and UI readers
   to use active-generation references consistently. Preserve internal validator
   routing tags while excluding those routes from ordinary user matching.
   Include interface/export caches, canonical command indices and qualified MCP
   discovery refresh at startup/restart and relevant Skill/Recipe activation.
   Existing advertisements/calls keep their listed contract; selected chat tasks
   keep their original snapshot. A refreshed list grants no execution authority.
5. Pin task selections and approval IDs once. Existing ordinary/review tasks,
   children and waits retain prior bodies/artifacts/evidence; new tasks receive the
   new generation. Current Tool settings are independently read at dispatch.
6. Treat corruption of an active generation as an explicit error to repair.
   Never add routing states called unapproved/unsupported match, downgrade silently,
   or remove a broken active workflow so Tier 2 can execute it instead.

Gate: concurrent activation yields one complete old/new generation; an old waiting
task continues its original code, while a live Tool block affects its next dispatch.

### Step 11 — Bootstrap, migration, upgrade and recovery

Bundled installation components, including validator and global-root code, do
not require an additional maintainer/human-Q2 approval. Development review and
the controlled seed integrity/automated behavioral qualification establish their
bootstrap provenance. Authored edits on an installed instance retain human Q2.
Missing seed producer/reader support must be implemented, not treated as an
external approval dependency.

Implementation evidence (2026-10-08): ordinary shared startup now retains the
exact packaged global-root draft idempotently. Both legacy seeders retain pending
review requests, preserve existing review state/feedback and recover missing
requests after an earlier component insert. They no longer manufacture Q1 passes
or builtin Q2 receipts. Protected Skill-table records retain their actual class.
Queue recovery propagates write errors. Three real native startup/recovery cases,
twenty PostgreSQL review cases and affected strict all-target linting passed;
details are in `docs/plans/simplified-v3-implementation.md`. This is a partial
step-11 repair, not the complete seed evidence adapter or authored catalogue
activation. Ordinary global-service wiring now exists; this historical evidence
does not qualify the new interface loader, validator admission or full library.
Historical receipts remain immutable legacy records.

1. Establish the controlled seed trust anchor: exact packaged validator/primitive
   manifests, verified source/artifact integrity and required observed behavior.
   Bootstrap cannot require a nonexistent already-active validator, but its
   exemption must be a verified system_seed contract with real evidence.
2. Reuse immutable evidence storage and implement the system-seed reader/writer
   provenance adapter. The current authored reader explicitly rejects that mode;
   a mode flag alone cannot change this. Only this controlled path permits null Q2.
3. Preserve the completed removal of fabricated builtin passes and required error
   propagation. Add actual controlled seed evidence and trace recovery/queue behavior
   and both builtin/zencoder seeders before changing boot. Admin diagnostics must remain
   available where the startup contract permits; unverified ordinary work stays closed.
4. Extend component integrity to cover validator Recipe layout/bindings and complete
   executable dependencies/artifacts, not only selected prose checksums.
5. Provide idempotent upgrade mapping from legacy validator UUIDs and pending review
   rows. Preserve old artifacts and evidence; identify unsealed/incomplete reviews
   as needing review. Do not manufacture approval from successful historical seeding.
6. Preserve operator overrides/deactivated routes across boot and repair. A seed
   upgrade stages a reviewed successor; it cannot overwrite an override or silently
   reactivate a disabled entry. Root-code changes require supervisor reconciliation.
7. Recovery fences cancelled/stale service generations and retains dispatch intent,
   counts, keys/arguments and confirmed/unresolved effects. VM memory is not a
   checkpoint. Unknown outcomes need explicit reconciliation; no whole-review or
   whole-Recipe replay and no parallel replacement global VM.
8. Backup/restore includes revision/evidence/activation references and required
   implementation artifacts. Test a real upgrade and restore before cutover; never
   alter previously applied migrations. Allocate new migration numbers at implementation.

Gate: fresh/existing boot, repeated boot, upgrade with overrides, interrupted
review/commit and restore preserve trust and task retention without invented passes.

### Step 12 — WebUI/API and production cutover

1. Reuse the working-tree association-review panel and API where their actual
   scope fits; verify feature gates, operator ingress and backend evidence checks.
   Show draft revision, active revision, exact validator selection, check failures,
   missing support/evidence, behavior results and human Q2 separately. Show the
   reviewed combination, not a display recomputed from latest component rows.
   Display retained interface/export/canonical execution Recipe identities and
   distinguish installed authored human review from bundled seed qualification.
2. Preserve meaningful pending/failed/incomplete/rejected states and resubmission.
   Disable approval when backend gates are incomplete; the UI is not the gate.
3. Support operator editing of validator Recipes, associated code and review
   prompts through immutable versions and the same Q1/behavior/Q2 contract. Do
   not expose arbitrary success-evidence writes or mutable approved-body editing.
4. Coordinate the global factory/startup, resources, continuation and artifact
   identity gates with simplified_v3 Phase 3a. Reuse the existing ordinary shared
   startup/owner; implement remaining review admission and interface loading there.
   Do not recreate the removed per-chat driver, add a parallel validator runtime or
   certify full catalogue/resource support from packaged reply/history evidence.
   Verify the actual selected Monty/control artifact and effective limits, including
   the current 512 MiB heap floor; do not introduce periodic pressure monitoring or
   artificial token caps while token budgets are disabled.
5. Perform the production acceptance matrix below. Only then replace the legacy
   Q1 entry path, retire its marker preparation/copied test bodies and obsolete
   comments, and remove the unused Rust audit workflow after caller verification.
6. Update current validator/queue/docs/UI instructions, retaining historical test
   reports as historical evidence. Report the accepted code/generation and remaining
   unrelated simplified-v3 work without declaring the entire v3 plan complete.

Gate: a real installed WebUI/Sempai proposal completes Q1/behavior/human Q2 and
becomes usable by the next ordinary task. Separately, a shipped validator upgrade
qualifies through the controlled seed path without instance Q2 and preserves
overrides/old selections. Eligible MCP exposure additionally passes the composition
plan's discovery -> completed command -> ordinary chat -> correlated reply/close
acceptance; no direct MCP execution bridge or new normal-chat runner is introduced.

## 5. Required acceptance matrix

Tests must exercise the relevant real PostgreSQL/packaged Monty/host/kernel callers;
helper-only fixtures do not prove production wiring. Expected source-derived results
are distinct from observed behavior. Run focused evidence under development policy.

| Case | Required observed outcome |
| --- | --- |
| Exact seed body | Regression catches the comment-marker failure; replacement consumes data without source modification. |
| All eight routes | Valid candidate reaches required gates; distinct nonempty invalid candidate is rejected for its class defect. |
| Metadata/source separation | Hostile strings, inert markers and candidate code remain data during inspection; source checksum is unchanged. |
| Full Recipe inspection | Empty steps, bad links/ordinals/classes, multiple UUIDs, unpaired bindings and invalid result edges fail. |
| Complete Skill | Missing association, multi-Tool purpose and prose/code disagreement do not pass final approval. |
| Interface integrity/selection | Changing only an export/signature/fixed argument changes the reviewed Skill content/combination; undeclared symbols, incompatible aliases and ambiguous multiple exports fail before effects. Association formats remain unchanged. |
| Effect-free preload | Load causes zero Tool/model/I/O/scheduling effects. Effectful defaults, decorators/initializers and unsupported module loading fail before invocation; load success is not usage success. |
| Deterministic export loading | Dependency-first order with stable-UUID ties is repeatable; missing helpers, cycles, conflicting names and undeclared imports fail. Pure helpers receive no artificial Skill/binding. |
| Function inspection/result capture | Reachable helpers and hidden host calls are checked against the owning usage. Explicit return values are captured/validated at invocation; a definition needs no fabricated top-level result assignment. Unsupported dynamic targets cannot bypass inspection. |
| Mutable invocation isolation | Repeated/concurrent invocations get fresh mutable defaults/arguments/arrays/results. Shared mutable globals/caches and closures over foreign inputs/host context fail; children/retries/waits cannot inherit unintended mutable state. |
| Export/host generation coexistence | Activate a successor during an old task's wait: both use their own interface/code/host implementation. Old aliases do not resolve through the newest feed; unsupported old-artifact recovery blocks the upgrade rather than replacing the handler. |
| Canonical Skill Recipe | Exact command/captures/types/fixed arguments select the intended pinned export with adjacent binding and real completion/result. Missing or mismatched canonical variants fail qualification; equivalent existing variants are reused. |
| Eligible MCP projection | Only available approved mcp-call-skill-recipes are advertised; exact command positions/types/escaping/examples agree with matching. Private validators/raw Skill rows are excluded. List refresh preserves existing advertised contracts; no per-call Q1/Q2 or direct execution is introduced. Public serving acceptance follows the composition plan. |
| Tool/ToolSkill compatibility | Missing callable/artifact, wrong adapter/selector/schema and conflicting aliases fail before effects. |
| Recursive schemas | Nested errors, missing versus null, invalid/default output fabrication, extra fields, booleans and numeric overflow fail precisely. |
| Internal assembly/chains | Valid supported graph executes with all revisions pinned; cycles/conflicts and uncovered/independent calls fail. |
| Multi-step state/wait | Producer results survive child execution/wait; interleaved review/task values never mix. |
| Conditional review coverage | Qualified branches/occurrences stop on defects or run required reasoning exactly as declared. Skipped/missing/interrupted required checks remain incomplete, and a straight skeleton cannot qualify unsupported control flow. |
| Tier restrictions | Deterministic route has zero model calls; model steps are explicit; shell/spawn_subagent stays Tier 1. |
| Exact review/evidence | Candidate or evidence swaps cannot graduate; validator upgrade during review preserves the original selection. |
| Missing/error infrastructure | No validator, ambiguity, SQL/provider errors or malformed verdict never invent a pass or enter Tier 2. |
| Actual behavior | Success/boundary/negative observations match expectations; missing tests, incomplete observations and mismatches cannot approve. |
| Human/provenance | Fake human/system flags, old receipts and self-issued evidence fail; real human covers exact Q1/behavior records. |
| Validator successor | Existing approved route reviews a new validator; successor never approves itself or silently changes in-flight reviews. |
| Draft containment | Draft execution exists only in constrained review; ordinary matching sees only the coherent active approved catalogue. |
| Live policy/cancel | Block after binding/wait, stale claim and cancellation prevent subsequent effects/replies even with old approved code. |
| Authoritative host failures | Caught kernel denial, stale/cancelled dispatch, exhausted resources or unresolved effects remain authoritative despite a success-shaped Python return. Only qualified recoverable outcomes may continue. |
| Effects/recovery | Timeout remains unknown absent evidence; completed effects are not replayed after output/reply/commit failure; counts survive reclaim. |
| Coherent publication | Concurrent edits/activation and prefix/cache failures leave a consistent old/new generation with no partial routing. |
| Lifecycle/migration | Fresh/repeated boot, overrides, pending review upgrade, supervisor recovery and backup/restore preserve required identities/artifacts. |
| Authored production end to end | Actual installed submission -> pinned validator exports -> evidence -> human Q2 -> publication -> next matched task works through existing production factories. |
| Bundled production end to end | Exact reviewed installation/upgrade package -> noncircular integrity/Q1/behavior qualification -> system_seed records with null Q2 -> coherent publication -> next task. No external/instance human approval is added; installed edits cannot impersonate this provenance. |

## 6. Evidence and stopping rules

- Before Rust build/test/check/clippy, follow root disk-space and target-directory
  rules. Reuse `/Users/ollama/brassclaw-target` and serialize shared-target builds.
  Read `LOCAL_TEST_ENV.md` before remote/provider tests; do not install another
  test infrastructure when the native PostgreSQL and packaged worker suffice.
- For each slice record the changed contract, caller, commit/diff, features,
  toolchain, observed results and limits. Run affected-package focused regressions
  and relevant linting; expand only for shared-consumer or unresolved acceptance needs.
- Native storage/evidence integration is required for transactional claims.
  Actual runtime admission and dispatch plus authenticated human actions for
  installed authoring, or controlled installation-owner qualification for shipped
  packages, are required for production approval/activation claims. Do not
  fabricate successful fixtures as evidence.
- Stop a slice after its coherent change and required checks pass. Do not rerun
  unrelated tests because the plan was edited. Final cutover still requires the
  coordinated subsystem acceptance in simplified_v3, including technical limits.
- For this plan itself, verify relative links, source references, dependency order
  and `git diff --check`; do not run Cargo solely for Markdown.
  Section 7's examples are design instructions, not observed execution evidence.

Final completion requires the applicable gates above, supported approval/provenance
for every activated validator/interface/dependency/canonical execution variant,
and both authored and bundled production cases. Public MCP serving is a coordinated
exposure gate, not a prerequisite for internal validators. Until then keep
drafts/pending migration visible and report support honestly.

## 7. Monty design and coding instructions

Read this section with steps 1–12. An implementer should be able to identify the
owner to change, the behavior to preserve and the test that proves each change.
Examples below specify proposed component behavior; they are not new supported
database/API fields, activated components or executable acceptance evidence.

### 7.1 — Freeze the actual interpreter contract first

The complete repository Markdown reference is retained under
`scripts/prefix/monty-reference/v1.0.0`: 59 documentation Markdown files,
including the API-generation templates and contributor page, plus license and
navigation metadata. The manifest's 61 file hashes were verified in this audit.
The read covers all quickstarts, concepts, CLI/examples/alternatives/commercial
server documentation and the complete limitations directory. The Python API
files are rendering templates, not the generated method documentation; check
the implementing source for any API used. Reading a template is not evidence
that its generated APIs are present in BrassClaw.

Record these identities in implementation evidence:

| Surface | Identity and implementation order |
| --- | --- |
| Pinned upstream documentation | `v1.0.0`, commit `85c5d1f6bef038405cfc40a4eed94806e303567e`; verify manifest hashes before relying on examples. |
| Application interpreter | `vendor/monty-control`, extension `1.0.0-brassclaw.control.7`; verify actual Cargo dependency, worker executable and package integrity. |
| Actual dump compatibility | `vendor/monty-control/crates/monty/src/dump_format.rs`: current/minimum supported version `0xBC05`. Earlier `0xBC04` paragraphs in the extension document are historical. |
| Actual production integration | `brassclaw_monty_host` uses the vendored interpreter/types/allocator and its own global worker transport. Upstream Python/JS and `monty-pool` APIs are separate surfaces. |
| Live documentation | Useful secondary comparison. Its [complex page](https://pydantic.dev/docs/monty/limitations/complex/) already documents support, while this pinned parser rejects complex constants. Never upgrade the supported profile merely because a new website example works. |

Before using an API, locate its definition, inspect its arguments, suspensions,
resource/error semantics and the production caller. Write that mapping into the
slice's evidence. Do not install Python/JavaScript packages, introduce a worker
pool, switch to Full Monty/WebSocket/CPython, or change the vendored interpreter
just to reproduce an upstream example. Those changes are separate architecture
decisions. Upstream performance figures are not BrassClaw measurements.

### 7.2 — Load definitions, then invoke through the suspendable task path

Implement in the existing retained source/compiler/interface and global Recipe
owners. Keep three operations distinct:

1. Inspect and qualify the exact source and dependency graph as data.
2. Load qualified definitions into the retained task's symbol environment.
   This must produce zero Tool calls, model calls, I/O or scheduled tasks.
3. After the matching ToolSkill binding, invoke the retained export with typed
   arguments and capture its return as the step result.

For example, a proposed one-Tool source-facts Skill could implement this function
after resolving a real existing inspection primitive, or implementing a missing
generic primitive through its supported registration path:

```python
# Associated PythonCode definitions; loading does not call the host.
def inspect_source_usage(*, source: str) -> dict:
    return host.inspect_source(source=source)
```

`host.inspect_source` above is an illustrative role, not a verified registered
callable. Step 1 must resolve the actual primitive, ToolSkill and UUIDs. Its job
is to return parser facts, not to decide which component class passes Q1.
Invocation has this shape after the loader/binding contracts are implemented:

```python
# Fixed, reviewed invocation boundary; runtime source is a typed string.
result = inspect_source_usage(source=inputs["source"])
```

The export name in that boundary is compiler-resolved from the retained interface,
not supplied by the candidate or looked up from a mutable global catalogue. If a
bridge source is needed, compile a fixed qualified invocation template and inject
only typed values. Never build `eval(export_name + "(" + user_text + ")")`.
Never execute candidate source to obtain its own structural facts.

The pinned `MontyRepl::call_function(name, args, print)` cannot drive this usage:
its implementation rejects kwargs and converts external/OS suspension into an
unsupported-context error. Extend the existing suspendable retained invocation
within `GlobalVm`/global Recipe transport. Do not add a parallel REPL or run a
function to completion on a second VM. A pure helper test may use a compatible
contained utility, but that is not production Tool invocation acceptance.

Do not introspect `fn.__name__`, `fn.__annotations__`, `inspect.signature` or
`callable(fn)` to validate exports; those Python facilities are unavailable in
the pinned subset. Validate source signatures and explicit interface records
before load, and resolve exact symbols in trusted compiler infrastructure.

Upstream external-function proxies retain a lookup name rather than an immutable
host implementation. Therefore the global host owner must resolve each actual
call using the retained task/occurrence binding context. Another task's newer
feed must not redirect an old function. Dispatch then applies current global
policy to that same stable Tool identity. Test old/new exports interleaved with
identical public names and different implementation revisions.

### 7.3 — Preserve typed data at every boundary

Reuse `value_contract.rs` and `brassclaw_monty_host/src/lib.rs::output_value`.
The current adapter already borrows `unstable::MontyNode` and rejects unsupported
kinds, non-string keys, nonfinite floats and oversized integers. Preserve those
checks for host arguments, results, nested values and any new invocation adapter.
Do not replace them with `repr()`, `to_string()` or Python/JS convenience conversion.

Upstream conversion can turn functions, iterators, classes and unsupported/deep
values into display strings; cycles can become markers. A string produced by
lossy conversion must not satisfy a declared string result. Reject the raw node
kind before conversion. A genuine string containing `"<deeply nested>"` is
ordinary valid data; do not reject strings by matching display-marker text.

Use this conceptual decision order, implemented through existing adapters:

```text
raw Monty node
  -> charge depth/node/byte budget before traversal/allocation
  -> require one supported data kind
  -> for number: require finite and representable by declared numeric contract
  -> for object: require string keys, reject duplicate keys
  -> recursively convert every child without truncation or coercion
  -> apply the exact producer result contract
  -> retain typed result for declared consumers
```

Recipe data is finite JSON-compatible data under recipe.md. The fact that Monty
supports bytes, tuples, sets, paths, datetimes, exceptions or proxies does not
extend that contract. Introduce a new transport profile only through explicit
schema/adapter/consumer qualification; do not silently encode bytes as strings
or tuples as lists. Numeric bounds must agree across DB, Rust, Monty and WebUI;
large JSON integers must not lose precision in a JavaScript Number round trip.

Parse raw structured documents with the existing strict Rust parser before
discarding duplicate keys. Monty's `json.loads` accepts NaN/Infinity and lacks
the hooks needed for this strict envelope policy. A later serde_json Value cannot
recover duplicate keys already overwritten. Do not use `hash()` or `repr()` for
checksums. Trusted owners hash the exact retained content/artifact bytes under
the established canonicalization contract; no `hashlib`/`hmac` exists in Monty.

Upstream conversion preserves repeated mutable references within one message.
It does not supply application isolation or immutable step results. Define typed
ownership in `retained_inputs` and the global invocation adapter: preserve required
within-invocation semantics, but copy consumer-owned mutable data rather than
letting a child or retry mutate another invocation or a retained producer result.
Check recursive immutability: `([1],)` is not immutable just because it is a tuple.
Do not share a `copy.deepcopy` memo across calls or assume host proxies are copyable.

Tests: bool supplied for integer; NaN/Infinity; deeply nested/cyclic output;
unsupported function/class/iterator output; non-string/duplicate object keys;
large integer round trip; aliased mutable inputs; child mutation; a genuine
display-marker string; exact boundary sizes. Excess rejects the whole operation.

### 7.4 — Qualify a bounded source profile without executing the candidate

Extend the selected Monty source-inspection utility and `retained_source.rs`,
not a separate CPython AST workflow. Monty cannot import `ast`, `inspect`,
`uuid`, `hashlib`, third-party libraries or arbitrary component files. Component
includes are IBS dependency assembly, not `import` from the host filesystem.

For the first qualified preload profile:

1. Allow explicitly declared function definitions and recursively immutable
   constants. Inspect every default, decorator, class-body expression and module
   initialization reachable during loading. Reject unsupported load constructs
   before execution; do not discover their effects by running them.
2. Resolve each call reachable from the selected export, including nested helpers,
   aliases and closures. Classify pure builtin calls separately from actual host
   dispatch. Reject unresolved dynamic targets under this profile. Record why a
   qualified construct is supported rather than weakening one-call checks globally.
3. Reject dynamic `eval`/`exec` in reviewed executable components under this initial
   profile; they compile unchecked source and change namespaces. Inert occurrences
   inside comments, docstrings or input data are not executable calls. A future
   supported dynamic profile needs separate evidence and cannot bypass Q1.
4. Use the pinned compiler for syntax/feature support. No inheritance, custom
   exceptions, `yield`, `match`, `del`, `async for`/`async with` or wildcard imports
   may be assumed. Supported classes/decorators are still not automatically safe
   to preload. Normalizing unsupported source into different code changes the
   reviewed artifact and is not a silent compatibility fix.
5. Charge and bound source inspection/compilation and fact generation before
   large allocations. Preserve the source scanner and native-stack constraints.
   Large comprehensions, generator expressions and `map`/`filter`/`zip`/`enumerate`
   may materialize whole lists in this interpreter. Do not assume CPython laziness.

A hidden effect in a default is a load failure, even though the file starts with
`def`:

```python
# Reject during qualification; loading evaluates this call.
def bad_usage(value=host.read_file(path="/data/input")):
    return value
```

This example contains an illustrative Tool name. The rule applies to every
actual Tool/model/OS/scheduling call, whether it is direct or hidden in a helper.
For mutable optional inputs use a qualified immutable default and construct local
state on invocation; the declared schema must explicitly permit that default:

```python
def collect_rule_ids(values=None):
    local_values = [] if values is None else values
    return [item["rule_id"] for item in local_values]
```

This is a pure helper, not a Skill. A null value cannot be used as a substitute
for missing data unless its contract permits null. Schema defaults apply at the
consumer's missing-input boundary, never to invalid outputs.

### 7.5 — Keep optional type checking separate from validation

The upstream type checker is optional and off by default. Annotations do not
validate runtime values. Passing static checking does not prove parser support,
actual result contracts, live policy, semantic agreement or correct behavior.

BrassClaw's current vendored dependency set does not include the upstream
`monty-type-checking`/`monty-pool` integration. Do not write `type_check=True`
into existing Rust constructors or claim a checker is bundled into this worker.
If static checking is added, first implement a supported primitive/integration
and qualify the exact checker, trimmed typeshed, stubs and versions.

Generate stubs only from the retained selected interfaces and host bindings.
Keep checker state isolated by review graph; reset reused checker files/state
between unrelated tasks. Upstream checking accumulates successful snippets and
can resolve stub-only modules which the runtime cannot import. Parser, structured
runtime contracts and observed behavior remain required even when typing passes.
A checker infrastructure failure is an incomplete review, not a candidate defect
or successful validation. A checker cannot generate trusted approval records.

### 7.6 — Keep Tool calls explicit and correlate every suspension

Callbacks executed synchronously by Monty cannot suspend. This includes
`sorted(key=...)`, `map`/`filter`, `min`/`max(key=...)`, `functools.reduce`,
`iter(callable, sentinel)`, iterator adaptors, default factories and several
implicit dunders. Reject a Tool usage hidden in such a callback under the
unsupported profile. Invoke Tool-using functions in ordinary suspendable
execution steps. A pure sorting key is fine.

```python
# Wrong: callback cannot suspend; also hides independent calls in one body.
results = list(map(inspect_source_usage, inputs["sources"]))
```

Implement repeated usages as the supported Recipe occurrence/control contract
from step 6.8, with one matching binding/invocation and durable effect identity
per occurrence. Do not invent iteration fields in Recipe JSON or a Rust loop that
decides the workflow. Pure bounded iteration over already supplied facts belongs
in reusable PythonCode.

For each suspension the existing host owner must:

1. Resolve the exact task, run, attempt, occurrence and retained binding from
   trusted admission state. Candidate data and trace baggage cannot select them.
2. Distinguish name lookup, Tool/OS call, pending future and control yield.
   A lookup prepares a value/reference; it is not proof a Tool was invoked.
3. Validate actual arguments, current policy, technical limits and freshness.
   Persist dispatch intent/count through the durable effect contract before effects.
4. Correlate the real host outcome to its exact pending call. Settle each once;
   refuse duplicates, stale generation answers and another task's IDs.
5. Resume through the supported continuation; preserve the separate authoritative
   effect/denial/cancellation state even if Python catches an ordinary exception.

Upstream asyncio supplies only `run`, `gather` and `sleep`. There is no
`create_task`, `Queue`, `Lock`, `TaskGroup` or `wait_for`. A coroutine is single-use.
Do not introduce a validator scheduling engine using APIs absent from Monty.
`gather` starts its children when awaited; already dispatched sibling calls can
still complete after another child fails. Cancellation/failure is not rollback.
Keep independent effectful validation steps sequential until the existing
occurrence/reconciliation contract explicitly qualifies concurrency.

Upstream `asyncio.sleep` begins waiting at the call, and zero sleep may not yield.
Use existing Recipe/task waits for product waiting semantics. Setting sleeps to
zero is a contained fixture option, never a way to implement real waits.
No task completion, cancellation or review error may shut down the global owner.

### 7.7 — Preserve task budgets and distinguish terminal engine failures

Reuse `SharedMontyTaskBudget`, the global execution/preparation observations and
the protected task interruption handshake in `brassclaw_monty_host`. Do not enforce
one review's quota with an engine-terminal control error that destroys every task.

| Limit or failure | Required design |
| --- | --- |
| Upstream feed/turn duration | Resets on new feeds/host rounds; not a durable task account. Keep the BrassClaw cumulative task compute account/revision across checks, child calls, waits and retries. |
| Compilation/value adaptation | Upstream feed limits omit compilation; the local control extension observes preparation separately. Preserve executing versus preparation clocks and their qualified ownership/accounting; do not silently change the 600-second executing-time target. |
| Shared VM heap | Use the actual tagged allocator domain and effective live ceiling. Preserve the 512 MiB instance floor and finite physical worker backstop. This is live allocation accounting, not RSS or reserved memory. |
| Host-side data | Bound parser facts, inputs/outputs, transport decoding, print capture, host callbacks and retained source/cache growth separately. A VM heap ceiling does not cover all these allocations. |
| Suspension count | Direct interpreter ResourceTracker does not implement the upstream pool's suspension-count enforcement. Trace existing host accounting, add missing per-task/review counters where needed, and retain them through reclaim. Do not apply a checkout's default 1000 to the entire lifetime of the global service. |
| Native operation/compiler hang | Keep the existing finite worker containment/watchdog. Checkpoints do not preempt every native callback; host deadlines are separate and cannot prove that an effect did not happen. |
| Genuine engine memory/time/control corruption | Fence the generation, stop new admission/dispatch and use supervised reconciliation before replacing the single global VM. A later successful feed is not proof its heap is safe. |
| Ordinary Python/domain exception or bounded output rejection | Fail the affected occurrence/review honestly; use the qualified task-local interruption path where needed. Do not classify every exception named MemoryError/TimeoutError as engine corruption. |

Retain structured failure origin from the VM/host owner. A manually raised
`MemoryError`, a print collector cap and a real allocator failure can have similar
display text but different continuation safety. Never classify by matching the
exception message. The current print collector is outside the VM heap; retain
its finite cap and attribute output to the task. Printed text is diagnostics,
not verdict/evidence. Assertion introspection may include operand values, so
avoid emitting secrets and do not use assert-message text as a stable rule ID.

If an instance-terminal failure occurs after a Tool effect, record confirmed or
unresolved effect state before attempting recovery. No blanket Recipe replay,
fresh retry counts or Tier-2 fallback is permitted. Logical task exhaustion and
instance recovery must remain distinct in tests and UI state.

### 7.8 — Make recovery trust and compatibility explicit

Coordinate continuation owners, retained task manifests, dispatch/effect records
and the global supervisor before implementing any dump/restore path:

1. Authenticate the producer/storage before deserialization. A caller-provided
   checksum next to caller-provided bytes is not trusted provenance. Do not accept
   arbitrary candidate/model uploads as VM state; serde decoding is not validation.
2. Verify exact compatible engine/control build, dump kind and `0xBC05` contract.
   Restore only through the qualified single-owner recovery path after fencing
   the old generation. Upstream examples that fork dumps into multiple sessions
   are not the BrassClaw global architecture.
3. Reattach trusted control/account ownership; the control Arc is not serialized.
   Restore durable task compute/counts, cancellation/freshness and pending effects.
   Snapshot-carried settings cannot override current live policy/effective settings.
4. Reconstruct retained host bindings and artifact identity from trusted task
   selection. Mounts, overlay changes, host-object registries, pending host
   coroutines and trace contexts are not a complete durable task checkpoint.
5. A restored call announcement is the same logical dispatch, not a new attempt.
   Reconcile its durable intent/receipt. Supply an already-confirmed answer once;
   keep unknown effects unresolved unless a qualified reconciliation establishes
   the outcome. Do not automatically call the Tool again.
6. Reject an upgrade that strands retained continuations/artifacts. Although
   upstream docs suggest replaying feeds after incompatible dumps, BrassClaw
   cannot replay effects. Drain/reconcile or retain a qualified recovery path.

These orders describe the required recovery gate; they do not claim that dumping
the current GlobalVm implements it. If its actual continuation cannot retain the
required state, implement that owner before advertising durable resume support.

### 7.9 — Keep the validator's host surface narrow

Provide bounded typed candidate/context/parser facts, approved selected usages
and explicitly declared testing resources. Do not grant ambient filesystem,
environment, clock, entropy or arbitrary host-object access for convenience.
An eager JSON fact has no lazy host property. A ClassInstance/ClassType property,
method or constructor can execute host code and retain host memory outside the
VM heap; it is not automatically a pure data read.

Monty defaults include the system clock and OS entropy. Deterministic Q1 checks
should consume declared facts and avoid time/random dependencies. If a rule needs
an instant or randomness, pass explicit typed fixture data or qualify task-local
OS policy/state; never change the shared module seed/clock for unrelated tasks.
Use the Rust monotonic task account for enforcement; Monty's `time.monotonic`
can be the wall clock, and a fixed clock never advances.

OS calls and mounts are additional host access mechanisms, not automatic
ToolSkill/kernel enforcement. Pure Q1 profiles must reject undeclared ones;
behavioral profiles may expose only qualified task-bound adapters checked by the
existing kernel. No `allowed_methods='all'`, broad mount or full process environment
is a shortcut to a binding. Reuse the protected host namespace rather than accepting
an input named `host` or letting eager inputs overwrite reserved exports/builtins.

Filesystem fixtures need special care: opening in write mode itself truncates,
reads may buffer the whole file, overlays reset per feed, and a cancelled host
write can complete later. Mount confinement does not guarantee bounded I/O time
or reverse an effect. Use explicitly controlled fixtures and durable effect
observation; never assert timeout/cancellation implies zero effects.

### 7.10 — Make all eight Recipes concrete without inventing storage fields

For each route author one ordered reusable workflow using step 7's full checks.
Resolve every role to an actual retained UUID and qualify its source/interface
before seeding. The following is a planning order, not persisted Recipe syntax:

```text
validate immutable subject/fact input contracts
-> obtain generic parser/graph/artifact facts through explicit bound usages
-> run shared identity/recursive-schema checks (pure PythonCode)
-> run this class's required checks (pure checks and explicit usages)
-> perform any declared semantic/model check through an explicit model usage
-> aggregate complete rule results (pure PythonCode)
-> retain trusted executor verdict/observations through the evidence owner
```

Behavioral qualification then tests the proposed usage/Recipe under the contained
review contract. Structural source facts alone do not establish that behavior.
The evidence writer authenticates executor provenance; it does not independently
choose validation sequencing or accept public `passed: true` JSON as evidence.

Use the persisted schema from recipe.md for component steps. This two-step
fragment illustrates a Tool binding followed by invocation:

```json
{
  "steps": [
    {
      "stepnumber": 1, "knowledge": "rust", "type": "component",
      "goal": "Prepare the selected source-facts binding",
      "content": "Binding metadata only",
      "include": ["<RESOLVED_SOURCE_FACTS_TOOLSKILL_UUID>"],
      "tool_bindings": [], "dependencies": null
    },
    {
      "stepnumber": 2, "knowledge": "orchestrator", "type": "component",
      "goal": "Invoke the pinned source-facts export",
      "content": "Invoke the associated PythonCode through its selected interface",
      "include": ["<RESOLVED_SOURCE_FACTS_PYTHONCODE_UUID>"],
      "tool_bindings": [], "dependencies": null
    }
  ]
}
```

This fragment is not a complete insertable Recipe. Put it in the actual
StepDescriptionEntry, add supported variants/step_link/input mappings and resolve
the placeholder UUIDs. It does not configure a binding through `content` or
automatically choose an export. Step 5a/6 must implement the selected interface
and typed layout in the actual owning contracts. Do not add `export`, `channel`,
`step_id`, revision or branch fields to persisted JSON to conceal missing support.

For input metadata `{{vars.source}}` is a whole-value reference; Python uses
`inputs["source"]`. `"prefix {{vars.source}}"` is not a valid reference. A real
input string containing that text remains data and is never reparsed as metadata.

Give each class an explicit rejection fixture in addition to shared cases:

| Route | Nonempty candidate that must not pass |
| --- | --- |
| 0 / Tool | Metadata/checksum present but selected implementation is not actually registered/loadable, or an alias maps to a different Tool policy identity. |
| 1 / Skill | Prose/export declares inclusive line interval while code excludes the end; schemas agree but behavior/meaning does not. |
| 2 / Skill | A domain overview secretly sequences independent file/network calls rather than one usage; migrate it to Recipe/Extension structure. |
| 3 / Skill | A model-usage export hides an undeclared second operation or omits the actual model failure/result contract; class label does not make it valid. |
| 13 / ToolSkill | Metadata calls the Tool or grants permission, or its selector/parameter adapter disagrees with the paired export. |
| 21 / Recipe | Valid-looking steps contain a forward result reference, missing binding adjacency, undeclared export selection or unsupported required branch/wait. |
| 22 / PythonCode | Default expression performs a host call, nested helper hides an independent Tool, or a mutable global leaks values across invocations. |
| 23 / ExtensionCatalogue | Referenced workflow UUID has the wrong class, is missing from its exact inventory or contradicts the declared task group/navigation contract. |

Implement aggregation as a reusable pure component. For example, after choosing
and implementing its strict input/result contract:

```python
def aggregate_checks(checks, required_rule_ids):
    seen = set()
    errors = []
    for check in checks:
        rule_id = check["rule_id"]
        if rule_id in seen:
            raise ValueError("duplicate rule result")
        seen.add(rule_id)
        if type(check["ok"]) is not bool:
            raise ValueError("rule result must be boolean")
        if check["ok"] is False:
            errors.extend(check["errors"])
    if seen != set(required_rule_ids):
        raise ValueError("incomplete rule coverage")
    return {"pass": len(errors) == 0, "errors": errors}
```

The runtime contract must reject duplicate required IDs, failed checks with empty
diagnostics, successful checks with errors, invalid diagnostics and unknown fields
before this function. The required rule list comes from the pinned validator,
never the candidate. Unknown/skipped/interrupted checks cannot become success.
`{"pass", "errors"}` is an illustrative component result shape here; the trusted
evidence owner still verifies complete required execution and authentic provenance.
This pure function does not write approval or need a fabricated zero-Tool Skill.

### 7.11 — Add focused interpreter-boundary acceptance

Add these to section 5's required matrix, at the owning caller rather than only
testing copied snippets under CPython:

| Case | Required outcome |
| --- | --- |
| Keyword export with real Tool suspension | Existing global runner invokes the pinned export; args/kwargs, return and external wait are preserved. No call_function/second-VM shortcut. |
| Source/data containing quotes, brackets, Unicode and markers | Candidate source/input is transferred unchanged as data. No source injection, accidental reference parsing or scanner failure from pasted values. |
| Callback reaches host | Qualification or supported runtime reports the unsupported context; no partial workflow pass or hidden independent dispatch. Pure callback fixture succeeds. |
| Two retained generations with same public names | Old task after wait invokes its old code/artifact; new task uses successor; both obey current live policy. |
| Output conversion loses information | Raw unsupported/cyclic/depth-truncated kind fails, while an actual string spelling a marker succeeds. No stringify repair. |
| Time budget across feeds/children/reclaim | Cumulative task account/revision does not reset; idle/external waits do not consume executing-time allowance. Preparation remains correctly attributed. |
| Suspension/print/host-data bounds | Per-task finite limits work across retries/reclaim without expiring the global instance after a checkout-sized count; host memory is bounded independently. |
| Failure origin | Ordinary Python exception/collector rejection stays distinct from genuine engine-terminal corruption. The task cannot swallow authoritative failure into a pass. |
| Interrupted gather/host write | Already dispatched effect is retained as confirmed/unresolved; siblings/cancellation cannot cause duplicate dispatch or fabricated zero-effects evidence. |
| Recovery pending host call | Re-announcement correlates to the original durable dispatch; confirmed answer is delivered once, unknown outcome requires reconciliation. Counts/pins remain. |
| Dump trust/ABI | Untrusted, wrong-kind and incompatible dump rejected before use; old generation fenced; no automatic feed replay or parallel VM. |
| Source position from older feed | Resolve filename against retained exact source; validate UTF-8 byte boundaries and exclusive end, including non-ASCII before the call. Never slice latest source by character offsets. |
| Optional checker state | If implemented, stubs/state from one graph cannot affect another; typing success still requires parser/runtime/behavior contracts. |

Source positions and tracebacks are diagnostic references, not authority. Store
the filename/feed-to-component-revision/checksum mapping with retained source.
No position or printed assertion proves a Tool ran or an approval exists.

### 7.12 — Implementation handoff and stopping criteria

Before changing code for a slice, write down four things: the current owner and
caller; the exact missing contract; the affected readers/transport/recovery/UI;
and the focused acceptance case. Reuse existing strict adapters, global lifecycle,
task accounts and submission storage wherever they already satisfy the contract.

Implement storage/reader/schema changes together, then source qualification and
loader/invocation support, then draft component composition, then observations and
trusted review wiring. Qualify bootstrap before publication. Never activate a
Recipe against an illustrative host name, missing export selector, unsupported
branch or future loader. A small model following this plan should stop that
slice's activation and implement the named missing owner, not improvise an API.

For each slice deliver the actual diff, contract examples, observed focused
tests and limitations. No Cargo run is needed for this documentation update.
Final deployment claims still require section 5 and simplified-v3 production
acceptance; this interpreter audit does not complete the architecture cutover.
