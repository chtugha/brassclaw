# Validator v3 implementation plan

Status: proposed implementation plan; no component activation or runtime acceptance.
Source audit: 2026-10-07. Reinspect the working tree before each implementation slice.

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

Completion means all eight class routes validate actual proposed immutable
components and relevant combinations through the real admission/IBS/composition/
Monty/host path, retain trustworthy evidence, feed human Q2, and permit coherent
activation only after all required gates. A validator verdict is not activation,
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
| Association/schema parsers and evidence readers exist | `association_contract.rs`, `value_contract.rs`, `association_review_store.rs`, V102 | Reuse structured formats; connect actual trusted producers and activation. Parsing declarations is not approval. |
| New structural/behavior producers exist separately | `pg_retained_q1.rs`, `retained_source.rs`, retained executor | Reuse observations and persistence; do not claim these execute the eight Recipes or establish human/semantic approval. |
| Working-tree human association review additions exist | `pg_association_review.rs`, `association_review_store.rs`, product facade and WebUI association-review panel | Reuse exact displayed-view checksums and transactional Q2/association writes after verifying their real ingress and tests. Their declared scope is one Tool usage; `catalogue_activated` remains false. They do not establish eight-class review or production activation. |
| Seed auditing directly records a trusted-root result and tolerates errors | `builtin_bootstrap.rs::audit_builtin_graduation`, `zencoder_bootstrap.rs` | Replace apparent bootstrap approval with verified seed provenance and required actual evidence. Review startup/recovery effects before tightening failures. |

The principal integration files are:

- Component stores/proposals: `crates/brassclaw_reborn_composition/src/pg_*_store.rs`,
  `sempai_proposal_sink.rs`, `pg_component_db_backend.rs`, `docplan_dissector.rs`.
- Review: `q1_orchestrator.rs`, `validation_queue.rs`, `validation_review.rs`,
  `pg_retained_q1.rs`, `crates/brassclaw_skills/src/association_review_store.rs`.
- Assembly/execution: `crates/brassclaw_engine/src/memory/{instruction_builder,
  composition,retained_instruction,retained_inputs,retained_tools,retained_reviews}.rs`,
  `crates/brassclaw_engine/src/executor/{retained_source,retained_recipe}.rs`,
  `crates/brassclaw_reborn_composition/src/global_recipe_ports.rs`.
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
Author/WebUI/Sempai proposal
  -> immutable draft revision + exact proposed dependency selection
  -> durable review submission
  -> select already-approved validator Recipe/variant and complete closure
     from one consistent catalogue generation
  -> IBS BuildInstruction -> typed composition and binding preparation
  -> global Monty executes reusable Q1 steps over candidate data
  -> trusted mechanism retains verdict, observations and exact review manifest
  -> constrained behavioral validation of the proposed usage/workflow
  -> authenticated human Q2 reviews the exact evidence set and combination
  -> trusted approval writer + coherent catalogue activation
  -> future tasks select the new generation; existing tasks keep their selection
```

Q1 and behavioral evidence may have separate records. Q2 must cover the required
successful records for the exact candidate/combination. A negative behavior case
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
   code/context/Tool graph, implementation artifacts and approval references.
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

## 4. Implementation sequence

Each numbered step has a deliverable and acceptance gate. Later steps may be
prepared as drafts, but must not activate against incomplete prerequisites.

Dependency order is explicit: steps 1–3 define the review subject and trust
contracts; steps 4–8 develop and test replacement workflows in the constrained
review environment; step 9 connects durable review and human decisions. Step 10
prepares publication but cannot publish the first new validator generation until
step 11 establishes its noncircular bootstrap trust and step 12 verifies the
coordinated production startup/admission path. Implement those prerequisites
before exercising publication, then run the final end-to-end activation case.
An existing validator may review successors only after its own provenance and
required evidence are verified; the legacy `validated` label is insufficient.

### Step 1 — Freeze contracts, inventory primitive reuse and define impact

1. Re-read the five guides, development policy and applicable subsystem rules.
   Record the commit and relevant working-tree changes; preserve operator edits.
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
2. Store a review submission pointing to the exact candidate revision/checksum,
   base revision if replacing, and proposed dependency graph. Do not project away
   schemas, bindings, includes, variant layout, effects or implementation identity.
3. Stage the draft and submission atomically through the authoring owner. Extend
   existing transactional staging; preserve compare-and-swap version allocation.
   Define an idempotent submission identity and unknown-commit recovery.
4. Migrate WebUI/Sempai/component-management writers to that path. Upgrades create
   new revisions; they must not overwrite the live approved row during Q2.
5. Map legacy proposals without inventing approval. Preserve exact originals and
   proposed values. Missing Skill associations/domain overviews become explicit
   migration work, not fabricated associations or silently reclassified approvals.
6. Define how a review chooses proposed draft dependencies versus already-approved
   dependencies. Never substitute latest or downgrade during the review. A new
   dependency combination needs combination evidence even if schemas are compatible.

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
3. Retain actual Tool artifacts/adapter identities for the validator's usages.
   `RetainedToolBinding` deliberately does not contain the executable artifact;
   implement and verify that missing resolver/retention boundary before dispatch.
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
   `result` assignment site does not prove every path returns the promised value.
6. Internal includes require real recursive resolution/assembly and checksum
   pinning. The current retained Tool program rejects nested code; extend its
   supported assembler and tests before approving validators that depend on it.
7. Likewise, implement permitted dependent-chain binding layouts explicitly.
   Until supported, those drafts cannot activate; do not redefine permitted v3
   chains as authoring defects or silently exclude an already-active workflow.

Gate: shared checks have positive/boundary/negative cases, parser facts are not
labelled semantic approval, and no class-specific workflow is hidden in a Tool.

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

Gate: a real multi-step validator transports typed results across an external
wait and interleaved task, preserves its snapshot, and cannot replay a completed
check's external effect after output/evidence persistence failure.

### Step 7 — Author all eight class-specific validator Recipes

Keep stable class routing, reuse common checks, resolve real component UUIDs,
and apply the following requirements. The names below identify existing routes;
they are not new Tool names or new constructor fields.

| Route | Required ordered checks and verification |
| --- | --- |
| `validator-class-0` / Tool | Identity and primitive grain; explicit operation/effect contract; registered schema versus Rust/host adapter; recursive inputs/results and transport limits; capability/callable/Tool UUID/alias mapping; retained artifact/checksum/ABI availability; actual errors/waits/cancellation; read-only/deduplication claims and constraints. Verification loads draft artifacts only in the constrained environment, measures declared behavior, and checks live policy across aliases/old versions. Compilation alone never passes the required review. |
| `validator-class-1` / Skill | Prose sections and one-usage meaning; exact `skill-association/1`; class-correct UUID relationships; all inputs, direct/computed arguments and actual callable compatibility; recursive output/error contract; exact failure metadata; code/internal graph. Automated semantic audits and behavior cases compare purpose/defaults/computation/effects to the prose. Q2 reviews what automation cannot establish. A permanent consuming Recipe is not required. |
| `validator-class-2` / Skill | The same complete usage checks as class 1. Correct the seed's obsolete domain-Skill label. Find multi-Tool domain overviews and migrate them with explicit reference updates into ExtensionCatalogue/Recipe structure; do not reinterpret class 2 as a hierarchy or mass-relabel existing rows. |
| `validator-class-3` / Skill | The same complete usage checks as class 1, plus verified explicit model-usage contracts where relevant. Class 3 alone neither executes an LLM nor makes a usage deterministic. Check actual model invocation, result/errors and consuming tier, without executing prose. |
| `validator-class-13` / ToolSkill | One non-executing/non-authorizing binding; Tool UUID/capability/callable/adapter/artifact agreement; registered parameter schema versus binding/usage contracts; fixed selectors and computed arguments; missing/default/null rules; recursive result/error expectations and numeric bounds; metadata include references/cycles/conflicts. Reject workflow/permission/retry execution in metadata. Test actual paired binding/call availability. |
| `validator-class-21` / Recipe | Parse actual persisted steps, variants and selected links; ordinals/classes/one UUID cardinality; matching binding pairs; typed capture/local-input mappings; complete dependency/code graph; result edges including optional/null/list guards; internal composition/call grain; tier restrictions; explicit retry/reconciliation/cancellation/final reply. Test every public variant with positive and negative routing/layout cases and whole-workflow effects/history/completion. Internal validator routes have an explicit typed entry point and remain excluded from public intent matching; document that distinction. |
| `validator-class-22` / PythonCode | Source parse and supported semantics; declared inputs/result and `result` behavior; no data-to-source substitution or forbidden imports/intrinsics; direct host-call facts against prepared associations; zero calls for pure logic; separate independent calls; supported covered dependent chains; recursive includes/order/symbol/input conflicts. Check assembled source and actual boundary/failure results. A pure-logic component needs no fabricated Skill/ToolSkill; Tool usages require their actual association. |
| `validator-class-23` / ExtensionCatalogue | Domain overview plus actual task-group/Recipe inventory; correct target classes and reference resolution; child IDs, dependency closure and intent/index consistency; no hidden executable workflow or replacement for usage contracts. Validate schema supported by current store/consumers and real retrieval/navigation, not invented catalogue fields. |

For all routes reject unknown/duplicate structured fields, nil/wrong-class UUIDs,
cycles, incompatible arguments/results, invalid defaults, bool-as-integer values,
unrepresentable numbers and unjustified retries where applicable. Keep shared
schema checks deterministic. Semantic checks must report scope/limitations rather
than pretend arbitrary Python correctness follows from parsing.

Gate: every route has distinct class-relevant negative fixtures; nonempty invalid
components no longer pass merely because their descriptions are present.

### Step 8 — Complete behavioral and semantic evidence

1. Reuse `pg_retained_q1.rs` actual structural observations and
   `BehavioralReviewSet` executor-owned data. Bind each record to the exact
   reviewed closure and validator/rule coverage; do not count the same observation
   as proof of a different graph or full Recipe completion.
2. Extend constrained review execution for pure logic, internal code and whole
   Recipes. Existing behavior producers cover selected Tool usages only. Verify
   outputs, effect counts, waits, reply/history/completion and negative outcomes
   at the appropriate level, without ordinary matching exposure of drafts.
3. Let Recipes define representative expectations/cases through approved contracts.
   Trusted execution measures the actual observations. Required case coverage
   cannot be omitted by a candidate submitting an empty test list.
4. Define reusable semantic-review usages for prose/code/binding agreement where
   model reasoning helps. Retain the reviewer/model/prompt components and exact
   output as evidence. LLM verdicts are fallible review evidence, not proofs or
   substitutes for mandatory behavior and human Q2.
5. Move changeable audit prompts/check sequencing from the orphaned
   `executor/code_audit.rs` workflow into approved review components where it
   supplies useful checks. Preserve hard protection against self-modification
   in the trusted boundary. Resolve class-10/50 root/scaffold review and supervisor
   reconciliation separately; generic Skill approval must never replace it.

Gate: an inclusive/exclusive interval mismatch with otherwise identical schemas
is caught by actual behavior/review; malformed source, denied policy, unknown
completion and failed downstream output each retain honest, non-replayed outcomes.

### Step 9 — Wire durable Q1 work, human Q2 and approval writers

1. Make submitted review work durable and discoverable via the existing admission/
   coordination services. Atomically retain submission and required work/outbox
   reference, or specify its idempotent reconciliation transaction before coding.
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
5. Pin task selections and approval IDs once. Existing ordinary/review tasks,
   children and waits retain prior bodies/artifacts/evidence; new tasks receive the
   new generation. Current Tool settings are independently read at dispatch.
6. Treat corruption of an active generation as an explicit error to repair.
   Never add routing states called unapproved/unsupported match, downgrade silently,
   or remove a broken active workflow so Tier 2 can execute it instead.

Gate: concurrent activation yields one complete old/new generation; an old waiting
task continues its original code, while a live Tool block affects its next dispatch.

### Step 11 — Bootstrap, migration, upgrade and recovery

1. Establish the controlled seed trust anchor: exact packaged validator/primitive
   manifests, verified source/artifact integrity and required observed behavior.
   Bootstrap cannot require a nonexistent already-active validator, but its
   exemption must be a verified system_seed contract with real evidence.
2. Reuse immutable evidence storage and implement the system-seed reader/writer
   provenance adapter. The current authored reader explicitly rejects that mode;
   a mode flag alone cannot change this. Only this controlled path permits null Q2.
3. Replace nonfatal builtin audit pass fabrication with actual verified evidence
   and required failure propagation. Trace recovery/queue behavior and both
   builtin/zencoder seeders before changing boot. Admin diagnostics must remain
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
2. Preserve meaningful pending/failed/incomplete/rejected states and resubmission.
   Disable approval when backend gates are incomplete; the UI is not the gate.
3. Support operator editing of validator Recipes, associated code and review
   prompts through immutable versions and the same Q1/behavior/Q2 contract. Do
   not expose arbitrary success-evidence writes or mutable approved-body editing.
4. Coordinate the global factory/startup, resources, continuation and artifact
   identity gates with simplified_v3 Phase 3a. Use one shared global orchestrator
   before workers/producers/ingress; no parallel validator runtime or legacy fallback.
5. Perform the production acceptance matrix below. Only then replace the legacy
   Q1 entry path, retire its marker preparation/copied test bodies and obsolete
   comments, and remove the unused Rust audit workflow after caller verification.
6. Update current validator/queue/docs/UI instructions, retaining historical test
   reports as historical evidence. Report the accepted code/generation and remaining
   unrelated simplified-v3 work without declaring the entire v3 plan complete.

Gate: a real WebUI/Sempai proposal completes Q1 and behavior, receives actual human
Q2 and becomes usable by the next ordinary task through the approved catalogue.

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
| Tool/ToolSkill compatibility | Missing callable/artifact, wrong adapter/selector/schema and conflicting aliases fail before effects. |
| Recursive schemas | Nested errors, missing versus null, invalid/default output fabrication, extra fields, booleans and numeric overflow fail precisely. |
| Internal assembly/chains | Valid supported graph executes with all revisions pinned; cycles/conflicts and uncovered/independent calls fail. |
| Multi-step state/wait | Producer results survive child execution/wait; interleaved review/task values never mix. |
| Tier restrictions | Deterministic route has zero model calls; model steps are explicit; shell/spawn_subagent stays Tier 1. |
| Exact review/evidence | Candidate or evidence swaps cannot graduate; validator upgrade during review preserves the original selection. |
| Missing/error infrastructure | No validator, ambiguity, SQL/provider errors or malformed verdict never invent a pass or enter Tier 2. |
| Actual behavior | Success/boundary/negative observations match expectations; missing tests, incomplete observations and mismatches cannot approve. |
| Human/provenance | Fake human/system flags, old receipts and self-issued evidence fail; real human covers exact Q1/behavior records. |
| Validator successor | Existing approved route reviews a new validator; successor never approves itself or silently changes in-flight reviews. |
| Draft containment | Draft execution exists only in constrained review; ordinary matching sees only the coherent active approved catalogue. |
| Live policy/cancel | Block after binding/wait, stale claim and cancellation prevent subsequent effects/replies even with old approved code. |
| Effects/recovery | Timeout remains unknown absent evidence; completed effects are not replayed after output/reply/commit failure; counts survive reclaim. |
| Coherent publication | Concurrent edits/activation and prefix/cache failures leave a consistent old/new generation with no partial routing. |
| Lifecycle/migration | Fresh/repeated boot, overrides, pending review upgrade, supervisor recovery and backup/restore preserve required identities/artifacts. |
| Production end to end | Actual submission -> validator Recipe -> evidence -> human Q2 -> publication -> next matched task works through production factories. |

## 6. Evidence and stopping rules

- Before Rust build/test/check/clippy, follow root disk-space and target-directory
  rules. Reuse `/Users/ollama/brassclaw-target` and serialize shared-target builds.
  Read `LOCAL_TEST_ENV.md` before remote/provider tests; do not install another
  test infrastructure when the native PostgreSQL and packaged worker suffice.
- For each slice record the changed contract, caller, commit/diff, features,
  toolchain, observed results and limits. Run affected-package focused regressions
  and relevant linting; expand only for shared-consumer or unresolved acceptance needs.
- Native storage/evidence integration is required for transactional claims.
  Actual runtime admission, dispatch and UI/human actions are required for
  production approval/activation claims. Do not fabricate successful fixtures as evidence.
- Stop a slice after its coherent change and required checks pass. Do not rerun
  unrelated tests because the plan was edited. Final cutover still requires the
  coordinated subsystem acceptance in simplified_v3, including technical limits.
- For this plan itself, verify relative links, source references, dependency order
  and `git diff --check`; do not run Cargo solely for Markdown.

Final completion requires all gates above, supported approval/provenance for every
activated validator and dependency, and the production matrix. Until then keep
drafts/pending migration visible and report support honestly.
