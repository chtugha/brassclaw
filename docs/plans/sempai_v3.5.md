# Sempai v3.5 — convert the current implementation into reusable v3 components

Date: 2026-10-10. Status: proposed; no implementation or deployment is authorized by
this document. The user requested this plan while keeping all current code,
components, data, configuration and deployments unchanged.

“v3.5” names this conversion work. It does not replace or relax the binding v3
architecture. Follow [recipe.md](../../recipe.md), [skills.md](../../skills.md),
[tools.md](../../tools.md), [toolskills.md](../../toolskills.md),
[simplified_v3.md](../../simplified_v3.md) and the
[development policy](../development-policy.md). Coordinate with the existing
[simplified-v3 implementation plan](simplified-v3-implementation.md) and
[prefix upgrade plan](prefix_v3_upgrade.md).

## 1. Main objective and success criteria

Reduce specialized code by building useful, general components that multiple
Recipes can reuse. Move evidence composition, proposal processing, iteration and
workflow completion decisions into small, explicitly associated PythonCode and
Recipes. Rust retains storage primitives, provider transport, integrity checks,
fencing, kernel enforcement and durable effect accounting.

The goal is not to minimize Recipe steps, move every check into Python, replace
Rust with a large Python workflow, or make the executor more elaborate for its own
sake. A reusable part has a coherent purpose, concrete contracts and at least one
credible additional consumer. Do not split every expression into a component or
introduce a generic “execute anything” Tool.

Distinguish three forms of reuse:

1. The same stable component UUID is referenced by different Recipes.
2. Small pure PythonCode parts are qualified dependencies of another PythonCode
   export, pinned and preloaded through the supported component loader.
3. A Recipe executes the same selected component repeatedly for separate items,
   with distinct invocation occurrences and durable effect identities.

The current flat executor restriction concerns item 3. It does **not** prohibit
items 1 or 2 and must not block extracting those reusable parts. Close a runner
support gap only when a concrete target Recipe needs that behavior.

Conversion is complete when the production Sempai Recipe uses shared building
blocks; Rust no longer contains its evidence-selection policy, proposal-processing
workflow, candidate-submission loop or workflow completion decision; all existing
safety and evidence guarantees remain enforced; and the shared parts run through
at least one additional Recipe without copied source or Sempai-specific adapters.

## 2. Current implementation to convert

Reinspect these files at implementation time; other work is changing the repository.
The observations below identify responsibilities, not immutable line numbers.

| Current source | Current responsibility | Conversion instruction |
| --- | --- | --- |
| [completed_turn_review_components.rs](../../crates/brassclaw_skills/src/completed_turn_review_components.rs) | Seeds a private Recipe, seven usages and mostly one-line Python wrappers around `host.review_*`. | Replace specialized wrappers with real reusable usages and pure transformations; keep installation seeding and integrity qualification. |
| [post_turn_request.py](../../crates/brassclaw_skills/src/post_turn_request.py) | Actual Python request construction from a claim, prefix, instructions and bundle. | Preserve its useful behavior; extract reusable message construction and envelope checks, with domain instructions supplied as pinned data. |
| [builtin_bootstrap.rs](../../crates/brassclaw_skills/src/builtin_bootstrap.rs) | Supplies packaged formatter/instruction sources for installation. | Continue controlled bootstrap. Runtime must read retained component revisions, not mutable source files or a fallback constant. |
| [backend.rs](../../crates/brassclaw_reborn_composition/src/completed_turn_review/backend.rs) | Reads evidence, composes/qualifies its bundle, enforces operation order, calls Sempai, decodes proposals, submits candidates in a Rust loop and acknowledges work. | Extract policy/composition/iteration into components. Retain or reuse only genuine primitive and technical enforcement responsibilities. |
| [completed_turn_analysis.rs](../../crates/brassclaw_skills/src/completed_turn_analysis.rs) | Strict response decoding, proposal policy, batch checks and host-derived submission IDs. | Separate generic strict parsing/integrity/identity services from Python interpretation of the Sempai response contract. |
| [ports.rs](../../crates/brassclaw_reborn_composition/src/completed_turn_review/ports.rs) | Task-bound dispatch, retained execution, operation intent/answer journaling and a fixed flat occurrence convention. | Preserve trusted boundaries; replace hardcoded Sempai operation order with validation against the pinned Recipe and actual occurrence records. |
| [delivery.rs](../../crates/brassclaw_reborn_composition/src/completed_turn_review/delivery.rs) | Journal delivery, provider/prefix pinning, native retention, accounting settlement and quarantine. | Keep delivery and resource ownership in Rust. It must not choose the Recipe's business steps or infer workflow success. |
| [mod.rs](../../crates/brassclaw_reborn_composition/src/completed_turn_review/mod.rs) | Bootstraps and retains the private packaged review graph/native implementations. | Qualify the successor shared component graph without replacing artifacts selected by existing tasks. |
| [global_mode.py](../../crates/brassclaw_engine/orchestrator/global_mode.py) | Existing global Python orchestrator and Recipe execution; Python No-Match model/Tool sequencing already exists. | Reuse the global service and Recipe interpreter. Do not replace already-correct Kohai sequencing with new Rust workflow logic. |
| [retained_inputs.rs](../../crates/brassclaw_engine/src/memory/retained_inputs.rs), [retained_recipe.rs](../../crates/brassclaw_engine/src/executor/retained_recipe.rs) | Currently prepare/enforce flat step occurrences despite broader Python flow interpretation. | Extend only the supported occurrence/flow contract required for real item iteration, with production-path proof. |
| [review_submission_store.rs](../../crates/brassclaw_skills/src/review_submission_store.rs) | Immutable candidate retention with exact bases/dependency closures and commit/conflict behavior. | Reuse its single-subject storage operation; expose a proper task Tool boundary if missing. Do not move transactional integrity into Python. |
| [pg_review_inspection.rs](../../crates/brassclaw_reborn_composition/src/pg_review_inspection.rs) | Operator evidence inspection and immutable quarantine observations. | Keep management in the existing facade. An observation does not release state, settle accounting or resume work. |

The existing Recipe is real, but many of its components only delegate substantial
workflow behavior to specialized Rust handlers. Generating Recipe/Python seed
records in Rust is legitimate; hiding workflow behavior in the seeded Tools is
what this plan corrects.

## 3. Nonnegotiable preservation requirements

- Exactly one supervised global Monty service. No second consumer VM, per-chat
  executor, Rust agent loop or Tier-2 fallback on errors.
- Only actual No-Match original turns produce this learning handoff. Preserve
  original outcomes, history/reply evidence and failures after publication.
- Never repeat the original model calls, Tools, reply or history to obtain evidence.
- Pin one coherent approved component graph, Recipe variant, step links, input
  layouts, exports, exact associations and actual Tool artifacts at task start.
  Child execution, waits and retries retain that selection.
- Pin the concrete Sempai provider/model and prefix/design/content identity at
  admission. Configuration changes affect future work, not an existing review.
- Preserve original/effective request bytes, actual response/failure bytes,
  provider replay identities, Tool arguments/results and unresolved effects.
- Runtime inputs remain typed data. No interpolation into Python source, `eval`,
  `exec`, filesystem imports or execution of component prose.
- Check current instance-wide Tool policy and technical constraints before every
  dispatch, independently of retained component approval.
- Preserve durable dispatch intents, positive attempt counts, idempotency keys,
  input bytes/checksums and confirmed/unknown effects across interruption.
- No automatic whole-Recipe replay, reservation reclaim, model redispatch or
  candidate replay after an unknown outcome. Initial conversion remains stop-only.
- Candidates remain unreviewed and unactivated. Parsing, retention, a model
  assertion or a summary cannot replace Q1, behavioral validation or authored Q2.
- Keep immutable V122/V127/V129/V133 evidence and retained native owners. Add
  successor migrations where needed; never edit applied migrations or erase old
  rows merely to make the new workflow fit.
- Token budgets remain disabled by default. Preserve actual model context limits
  and explicit technical transport/resource limits; reject whole oversized data
  rather than silently truncating evidence or proposals.

## 4. Target shared component library

These are **logical responsibilities**, not existing callable names, UUIDs,
accepted database fields or activation claims. Resolve actual existing components
first. Create a primitive only for a demonstrated missing system operation.

| Reusable part | Owner and one-operation boundary | Other useful consumer |
| --- | --- | --- |
| Read one immutable record by an allowed reference | Existing Rust store primitive plus one Skill/PythonCode usage | Incident review, component review, reconciliation inspection |
| Read a bounded conversation/history range | Rust transport/store query with correlation enforcement | Conversation export or reply/history auditing |
| Read retained model exchanges or effect receipts | Rust evidence store reads; no transcript policy | Provider diagnostics and effect reconciliation |
| Select a transcript boundary | Pure PythonCode over typed records and explicit boundaries | Completed-turn audit or bounded conversation summary |
| Correlate evidence and build completeness diagnostics | Pure PythonCode composed from small checks | Read-only evidence audit Recipe |
| Assemble an evidence envelope | Pure PythonCode; preserve exact raw byte strings alongside parsed data | Incident or component-validation evidence bundle |
| Construct messages from pinned prefix, instructions and evidence | Pure PythonCode with explicit role/order contracts | Prompt review, Recipe authoring, domain analysis |
| Call one selected model once | Shared Rust provider transport plus a one-usage Skill | Kohai explicit LLM step or another reviewer |
| Parse strict JSON into typed data | Existing/shared validator primitive where Monty lacks equivalent strict parsing | Configuration/protocol parsing |
| Interpret a proposal envelope and prepare one candidate | Pure PythonCode with explicit Sempai envelope policy | Other component-authoring Recipes sharing candidate preparation |
| Validate one immutable candidate/base/dependency selection | Shared read-only structural/integrity primitive | Operator authoring and validator Recipes |
| Retain one candidate subject | Rust transaction through existing submission store plus one Skill | Human/operator or another authoring Recipe |
| Aggregate actual receipts and build a completion result | Pure PythonCode | Other batch authoring/review Recipes |
| Append an immutable decision/result record | Rust storage primitive, no decision-making | Audit and supervised reconciliation Recipes |

A domain-specific formatter or policy component can still be useful. Its
specificity belongs in component code/data, rather than a new Rust workflow Tool.
Pure PythonCode is not a zero-Tool Skill. Each real Skill describes one Tool usage
and explicitly associates its executable export and compatible ToolSkill.

## 5. Ordered implementation steps

### Step 1 — capture the baseline and build a reuse map

- [ ] Read the four component guides, current subsystem contracts and development
  policy. Record the commit and relevant dirty changes without reverting them.
- [ ] Inventory all seven review operations, their native registrations, exact
  parameters/results, effects, durable records and failure paths.
- [ ] Search current Recipes, PythonCode, Skills, ToolSkills, validators and native
  adapters for each responsibility in section 4. Check signatures and actual
  supported registration, not just names.
- [ ] Produce a reuse map: existing UUID/export/Tool identity; retain unchanged;
  new version; or genuinely missing support with its precise justification.
- [ ] Preserve the current deployment and records. No cleanup or cutover yet.

Exit: every target part has an owning layer and a concrete reuse decision. Existing
consumer/integration evidence is recorded with its limits and current applicability.

### Step 2 — author contracts and the two consuming Recipes first

- [ ] Specify the Sempai review Recipe and a **read-only completed-turn evidence
  audit Recipe** that shares its readers, transcript selection, correlation and
  bundle composition. The audit Recipe makes no model call or candidate write.
- [ ] Write per-step inputs, result edges, failure actions and completion meanings.
  Supply recursive object/list schemas, missing/null semantics, bounds and defaults.
- [ ] Define reusable public exports, private helpers, qualified dependencies,
  actual Tool bindings and exact Skill associations.
- [ ] Persist Recipe steps using `knowledge`, `stepnumber` and one UUID in each
  component `include`. Each ToolSkill binding immediately precedes its matching
  class-22 invocation. Pure logic needs no artificial binding.
- [ ] Keep stable UUIDs in Recipe definitions; pin revisions/checksums in retained
  selections. Do not paste version numbers into Recipe references.
- [ ] Classify the review as Tier 1 because it calls a model. The read-only audit
  can be Tier 0 only after its deterministic eligibility is established.

Exit: the target is an explicit component workflow, not a proposal to replace seven
specialized Rust methods with one large specialized Python method.

### Step 3 — separate technical primitives from workflow policy

- [ ] Extract shared immutable reads, bounded history reads and effect/exchange
  reads from `Backend::qualify`; reuse existing service/store operations.
- [ ] Reuse the existing model gateway behind a one-call task Tool usage. Preserve
  provider pinning, context limits, cancellation, accounting and request capture.
- [ ] Expose single-candidate retention through the existing immutable submission
  store. Its Tool never accepts a whole Sempai response or loops over proposals.
- [ ] Keep strict parsing, checksums, exact artifact/base/dependency integrity and
  host-owned operation identities in reusable trusted primitives where required.
- [ ] Remove Sempai-specific instruction/message equality from the future generic
  model transport. Replace it with validation against the selected request
  contract and task-owned pinned source references. Do not weaken identity checks.
- [ ] Register actual implementations, adapters and retained artifacts. Canonical
  Tool identities and aliases must preserve live policy, including intentional
  blocks; ambiguous policy migrations fail closed.

Exit: no new primitive chooses which evidence is sufficient, which proposal to
process next, or whether the review has achieved its goal.

### Step 4 — implement only the necessary execution and journal support

Reusable pure code and cross-Recipe references can be converted without dynamic
iteration. Proceed with those extractions independently of this step's loop gate.

- [ ] For candidate iteration, inspect `RetainedRecipeInputs::monty_flow`,
  `RetainedRecipeExecution::run_step`, review port occurrence checks and the global
  Python flow interpreter. Current Python `branch`/`foreach` support alone is not
  proof that persisted Recipes and native execution accept them.
- [ ] If the selected path lacks the needed contract, implement validated,
  checksummed flow storage/reader support in the owning schema and IBS first.
  `recipe-flow/1` is currently prepared runtime data; do not invent an accepted
  Recipe INSERT field. Include the flow in qualification and retained selection.
- [ ] Replace flat `started.contains(step_id)` and `execute_<index>` assumptions
  only where necessary with pinned step plus unique occurrence identities.
  Validate membership, typed item bindings, legal result edges and occurrence
  freshness before dispatch. Rust verifies a requested occurrence; Python chooses
  the branch/item and next component.
- [ ] Preserve per-item/per-attempt state isolation, child selection, bounded
  memory/compute, waits and existing resource settings. No task recreates the VM.
- [ ] V129's operation key `(attempt_id, operation)` cannot represent repeated
  submissions. Add an immutable occurrence journal/extension with explicit links
  to legacy rows, exact intent/answer bytes, counts and effect identities.
- [ ] Do not simply relax duplicate-step rejection or append an iteration number
  supplied by the model. Derive/verify occurrences from the retained workflow and
  actual Monty execution; distinguish legitimate new items from effect replay.

Exit: one Recipe may invoke the same single-candidate usage for multiple items
safely. If this gate is incomplete, leave production batch submission unchanged;
do not conceal iteration in Rust or one multi-Tool Python body.

### Step 5 — move evidence selection, correlation and composition into PythonCode

- [ ] Build reusable pure exports for transcript selection, model-exchange
  correlation, Tool input/result correlation, diagnostics and bundle composition.
- [ ] Preserve current checks: source/run/turn/conversation identity; actual
  accepted-message and finalized-reply boundaries; failure after publication;
  withheld/redacted evidence; capture/dispatch counts; provider replay arguments;
  final model content versus actual published reply; unresolved invocation effects.
- [ ] Preserve available preceding conversation history through the selected
  terminal boundary. Exclude later turns. Do not equate daily memory with the
  complete transcript or manufacture missing evidence.
- [ ] Readers provide authoritative, correlated records. Pure components return
  typed diagnostics and a complete/incomplete outcome; the Recipe branches on it.
- [ ] Keep raw event/request/response/candidate bytes unchanged alongside parsed
  values. Deterministic ordering/serialization belongs in declared component code.
- [ ] Persist the actual qualified bundle and the actual selected qualification
  result through a storage usage. A caller/model-provided `evidence_complete:true`
  is not a trusted attestation: retain source refs, selected validator revisions,
  occurrence identity and checksums so later host checks can verify provenance.

Exit: `Backend::qualify` no longer selects transcript boundaries or constructs the
review bundle. The same Python exports work in the read-only audit Recipe.

### Step 6 — move claim/request construction into reusable usages

- [ ] Retain claim/reservation ownership and atomic once-only dispatch fencing in
  Rust. These are protected storage/transport operations, not model decisions.
- [ ] The Recipe selects the complete or incomplete path using actual qualified
  data. Incomplete evidence is retained and stops before any model dispatch.
- [ ] Refactor `prepare_post_turn_request` into shared message/envelope construction
  plus a small Sempai-specific composition. Domain instructions come from the
  selected retained component; no runtime fallback to seed constants.
- [ ] Preserve prefix/instruction/evidence role separation and untrusted-evidence
  treatment. Account for provider adapters merging adjacent system messages;
  retain the distinct host request and actual effective provider layout.
- [ ] A one-model-call Skill invokes the shared transport once with the pinned
  model route, prefix identity and constructed messages. Preserve original and
  effective requests, response/failure and actual usage before downstream work.

Exit: another reviewer Recipe can reuse message construction and the model-call
usage by supplying its own approved instructions, without a Sempai-specific Tool.

### Step 7 — move proposal interpretation into reusable PythonCode

- [ ] Keep duplicate-key-aware strict parsing and data limits. Do not replace
  `strict_json` with ordinary `json.loads` if it silently discards duplicate keys.
  Reuse a strict parse primitive if the selected Monty environment needs one.
- [ ] Python checks the Sempai envelope format, nonempty analysis, explicit
  `candidate_bytes`, mandatory `base` including null, exact dependencies, allowed
  candidate classes and duplicate candidate identities. Reject the entire invalid
  response; do not silently filter bad proposals.
- [ ] Separate generic candidate preparation from the domain response envelope.
  Preserve exact bytes and exact dependency refs; never execute proposed code.
- [ ] Preflight **all** proposed candidate/base/dependency graphs through reusable
  read-only validators before the first candidate write. Add missing read-only
  validation support in the existing store, not a new batch workflow Tool.
- [ ] Retain the complete validated list and response checksum. Empty proposals
  mean no candidates produced; no summary may claim an export occurred.
- [ ] Keep submission identity derivation host-owned. Preserve the existing
  domain-separated attempt/response/index/candidate-checksum algorithm or introduce
  an explicitly versioned compatible successor. Python carries host-issued IDs;
  a model cannot select an existing submission identity.

Exit: `completed_turn_analysis` contains only reusable technical parsing/integrity/
identity functions or retained legacy support; Sempai interpretation lives in
component code. Structural preflight is not behavioral Q1 or activation.

### Step 8 — move candidate iteration into Recipe execution

- [ ] After step 4's gate, use the supported Recipe iteration contract over the
  validated list. Each item invokes the same single-candidate Skill/PythonCode,
  with its immediately applicable ToolSkill binding and unique occurrence.
- [ ] Keep one candidate retention per invocation. Independent candidate writes
  require separate execution occurrences; they are not a dependent-chain exception.
- [ ] Rust's submission primitive retains one exact subject transactionally,
  checks actual bases/dependencies and returns an actual immutable receipt.
- [ ] Persist each intent and answer independently. Item failure retains earlier
  successful receipts; the Recipe stops without replaying them or asking the model
  again. Unknown commit uses supported exact-ID/readback reconciliation, not an
  automatic second model call, new ID or whole-Recipe rerun.
- [ ] Handle an empty list with an explicit no-candidates branch. Aggregate receipts
  in reusable pure PythonCode after the item loop; preserve proposal/receipt order
  and exact linkage to candidates.

Exit: remove the candidate `for` loop from the successor Rust backend. Successful,
partial and unknown outcomes remain distinguishable in the store and inspector.

### Step 9 — move workflow completion decisions into Python/Recipes

- [ ] Pure components compare validated proposals and actual retained receipts,
  returning typed no-candidates, submitted-unreviewed, incomplete or failed outcomes.
  Define any successor receipt format explicitly; do not overload legacy meanings.
- [ ] The Recipe decides which result record to persist and what completion value
  to return. Rust's write primitive validates integrity/provenance and atomically
  retains the record; it does not choose the business result or invent success.
- [ ] Keep native service settlement independent: actual root completion,
  accounting, withheld answers, closure/fencing and unresolved effects determine
  whether retention credits may be released. A Python result does not prove native
  quiescence, zero spend or safe resource release.
- [ ] Preserve truthful reporting when workflow acknowledgement succeeds but root
  settlement/accounting fails. Retain the owners and quarantine; do not rewrite
  original outcomes or treat an operator observation as reconciliation.
- [ ] Keep Q1/behavior/Q2/activation as separate supported Recipes and human
  management paths. This conversion never auto-activates Sempai candidates.

Exit: Rust can establish that execution settled safely, while Python/Recipes own
whether the workflow goal was achieved and how its result is expressed.

### Step 10 — qualify and seed the successor shared library

- [ ] Resolve stable component UUIDs using supported authoring/install identity
  allocation. Allocate new identities where primitive meaning differs; publish
  immutable successor revisions where identity and purpose remain compatible.
- [ ] Declare preloadable exports, private symbols, qualified dependencies and
  recursive contracts. Load definitions dependency-first without effects; invoke
  retained exports with typed `inputs`, without copying source between Recipes.
- [ ] Qualify exact Skill/PythonCode/ToolSkill/Tool combinations and native artifacts.
  Installation-owned shipped components use reviewed `system_seed` bootstrap with
  integrity, Q1 and behavioral evidence, without extra installed-instance Q2.
  Installed authored edits still require their proper Q1/behavior/human-Q2 path.
- [ ] Preserve operator overrides and old retained versions. Source/status labels
  and a manifest alone never establish trusted bootstrap or combination approval.
- [ ] Update `Package::boot` and retained component selection to the successor
  graph. Keep internal-only review usages out of public matching and MCP discovery.
  Reusable public usages need real canonical execution Recipes and accurate commands.

Exit: both target Recipes select one coherent qualified graph. No mutable callable,
latest lookup or generated wrapper substitutes for a retained approved export.

### Step 11 — validate reuse and safety through actual callers

- [ ] Run the shared evidence components through the read-only audit Recipe and
  the Sempai review Recipe. Verify the same component UUIDs/exports are selected
  without copied code; different task values remain isolated.
- [ ] Exercise real PostgreSQL, native kernel bindings and the existing global
  Monty service. Retain the existing positive review and matched/No-Match tests
  where still applicable; helper tests alone cannot certify conversion.
- [ ] Cover complete/missing/redacted evidence, later conversation messages,
  failure after reply publication, multiple original model/Tool iterations,
  unknown effects and malformed/duplicate-key/source-like data.
- [ ] Cover zero/one/multiple proposals, an invalid later proposal before any write,
  exact base/dependency conflicts, a later item failure, commit uncertainty,
  duplicate occurrence, stale generation, cancellation and live Tool blocking.
- [ ] Replace provider, prefix and component revisions during an admitted review;
  verify old work keeps its original selection and later work selects successors.
- [ ] Interrupt at dispatch intent, provider entry, response retention, candidate
  commit, acknowledgement and root settlement. Verify no effect or original turn
  is replayed, no attempt count resets, and unresolved owners remain retained.
- [ ] Run the existing operator inspection regression against old and successor
  evidence. Record execution evidence separately from observations and approval.
- [ ] Apply focused compilation/lint/integration checks under the development
  policy, including disk-space rules. Report baseline blockers honestly. Any
  real-model quality evaluation is additional evidence, not proof of safe recovery.

Exit: production-path behavior and cross-Recipe reuse both pass. No full-v3 or
“flawless model” claim follows from a provider fixture or a parsed candidate.

### Step 12 — cut over new admissions, then retire specialized code safely

- [ ] Before cutover, run comparison only over retained evidence with read-only
  pure transformations. Never shadow provider calls or candidate writes.
- [ ] Enable the qualified successor for future **unreserved** source events.
  Existing V129 reservations retain their original graph and native handlers;
  uncertain/failed work is not re-admitted under the successor.
- [ ] Preserve the original unique source-event delivery identity across both
  implementations. Atomic delivery fencing must prevent dual consumption.
- [ ] Keep a compatibility reader and inspector for existing events, operations,
  requests, receipts and settlements. Do not rewrite their exact bytes/statuses.
- [ ] Retire obsolete specialized wrappers, methods and registrations only after
  no retained task/checkpoint/quarantine owner needs them. Keep legacy artifacts
  as long as actual selections reference them; metadata alone is insufficient.
- [ ] Rollback changes only future selection/admission. It never replays old work,
  overwrites a committed candidate or destroys unresolved accounting/evidence.
- [ ] Update the runtime contract and linked plans with exact implemented support,
  test evidence and remaining gaps. Keep supervised effect/accounting reconciliation
  separately identified until its own supported path and acceptance exist.

Exit: the Sempai workflow is implemented through reusable v3 components in ordinary
startup, while historical evidence and in-flight work remain intact.

## 6. Required responsibility review before marking complete

For each remaining Rust method, answer: “Does this perform one storage/transport/
integrity/authority/accounting operation, or does it select and sequence domain
work?” Move the latter into components. For each Python export, answer: “Can another
Recipe use this with different typed inputs, without copying its implementation?”
Keep a justified domain-specific component where useful; do not generalize away
its meaning or hide independent Tool dispatches in an opaque helper.

The final review must specifically find no successor Rust implementation of:

- transcript selection plus bundle composition;
- Sempai envelope interpretation plus candidate-processing policy;
- iteration over multiple candidate retention effects;
- business completion classification based on model output;
- a hardcoded seven-operation workflow masquerading as generic host enforcement.

Rust must still enforce actual integrity, correlation, immutable retention,
transactional conflicts, dispatch fencing, current kernel policy and durable
accounting. Removing these protections would be an architectural regression.

## 7. Work deliberately left unchanged by creating this plan

This document does not change any Recipe, PythonCode, Rust implementation, seed,
migration, database row, provider, prefix, service, Docker Compose stack or remote
machine. It introduces no new host callable or activated catalogue entry. All
implementation checkboxes above remain open until that work is separately carried
out and verified.


## Native reconciliation implementation follow-up

The separate effect/accounting reconciliation work is documented in
[post-turn review reconciliation](../reborn/post-turn-review-reconciliation.md).
It adds native receipt custody, exact operation-answer recovery, opt-in accounting
readback, immutable batch-subject verification and guarded retention release, with
an operator management action. This is infrastructure/integrity work; it does not
convert the existing Sempai evidence, proposal or workflow policy into Recipes.
Keep those conversion steps open. The future shared usages must preserve the native
release predicates and must not treat a Python completion result, an operator note
or a stored reconciliation row as proof that lost native state was released.

## Component deployment implementation follow-up

The operator bridge from complete retained Recipe submissions to the coherent
catalogue activation service is documented in
[component deployment](../reborn/component-deployment.md). Preparation reads exact
subjects, reuses existing trusted usage approvals and preserves unrelated active
Recipes. Explicit activation retains subject pins and uses the existing immutable
receipt/CAS path; it does not create validation evidence or replay effects. This
management integration does not convert the Sempai authoring, validation or domain
workflow into reusable components. Keep those conversion steps open.
