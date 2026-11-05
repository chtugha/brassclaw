# Repair Bob 2 — catalogue, publication and retained readers at K3

Date: 2026-11-03. Status: **implementation plan; no publication or acceptance is implied**.
Reviewed committed baseline: `958cbb9c86d50644c59fcd40ced9c5a7cafc9cdc`.

## 1. Scope and ownership

Repair the existing catalogue/IBS/deployment/retained-selection chain so it
consumes genuine exact approvals and retains them throughout execution. This
implements the reader side of the existing Validator/Composition responsibilities;
it does not add a catalogue, approval service, provider store or execution engine.

Read [AGENTS.md](AGENTS.md), [development policy](docs/development-policy.md),
[recipe.md](recipe.md), [skills.md](skills.md), [tools.md](tools.md),
[toolskills.md](toolskills.md), [pure review](docs/reborn/pure-component-review.md#exact-remaining-cutover-contract),
[composition plan](docs/plans/monty.composition.md) and its
[implementation plan](docs/plans/monty.composition.implementation.md).
Own-functional A versus exact-usage S and immutable task selection remain binding.

| Input/output | Implementing plan | Boundary |
| --- | --- | --- |
| Trusted subject, observed Q1/behavior/Q2, shipped qualification and verified approval reference | [Plan 1](Repair_bob_1_v3_at_k3.md), B1 | This plan consumes the existing trusted owner's checked reference; never validates `succeeded` JSON itself |
| Coherent qualified selection, exact approval pins and all readers | This plan, B2 | Returns immutable selections through existing catalogue/IBS interfaces |
| Durable original-request scheduling, host lifecycle and Codex | [Plan 3](Repair_bob_3_v3_at_k3.md), B3 | Supplies admitted original work/claims; cannot grant approval or choose latest code |

B1 owns the shared dependency-stage order. B2-01/B2-02 can proceed with B1's
subject/verdict design. Positive publication waits for B1-05/B1-06's actual
verified qualification. B1-07 then consumes B2's qualified validator selector.
No profile is exposed until all applicable readers below understand it.

Follow B1's mandatory implementation/evidence quality rule throughout: actual
stores/host/kernel/retained programs, no stub or simulated successful backend,
and no new hardcoded catalogue/query/heap/token quota. Use acknowledged existing
whole-value/preparation settings and reject oversized values intact. Structural
format versions, UUID/checksum representation and supported call cardinality are
protocol contracts, not arbitrary resource defaults. Any necessary new setting
must include its normal DB/API/WebUI description and effective revision uptake.

The review's UUID-only pure check is repaired in B2-03; unconsumable shipped
records in B2-04; missing graph-coverage and retained-reader integration in
B2-03/B2-06/B2-07. B1 owns their evidence/identity producer repairs. B3 consumes
the resulting original selections without creating another reader or queue.

## 2. Reconcile actual source and build the complete reader inventory

- [x] **B2-01 — Record current producer-to-consumer paths.**

**Completed source inventory — 2026-11-03:** the detailed inventory below is
reconciled with the now committed receipt/admission changes in the
[current source map](docs/validation/2026-11-03-bob-committed-reconciliation.md).
Its pure-coverage, approval-pin, strict-reader and relationship findings remain
open implementation work, not reasons to recreate the repaired validator graph.

**Source inventory recorded — 2026-11-03:** the
[reader/format inventory](docs/reborn/bob2-retained-reader-inventory.md) records
the existing shared checker, exact selection owners, strict nested parsers, SQL
format gates and ordinary/child/private/Prefix/MCP/deployment consumers. It also
records the UUID-only pure lookup, preparation-versus-recheck disagreement,
missing pure pins and V201 relationship gap. No runtime/DB acceptance or successor
activation follows from this source audit. Reconcile concurrent validator edits
and their current evidence before closing the complete B2-01 gate or editing
their consumers. B1 remains the trusted producer owner.

Re-read current HEAD and affected uncommitted diffs before edits. The reviewed
tree contained ongoing validator/bootstrap/runtime work; do not replace it with
the committed baseline or duplicate its repaired constructor. Record hashes,
existing accepted evidence and unresolved behavior by path/feature/backend.

Start with these real owners:

- `crates/brassclaw_reborn_composition/src/{authored_monty_catalogue,installed_monty_catalogue,component_deployment,pg_pure_component_approval,pg_association_review,pg_recipe_store}.rs`;
- composition `{mcp_command_qualification,mcp_recipe_catalogue,mcp_chat_bridge,global_recipe_ports,global_monty_startup}.rs` and selected child/private/prefix admission callers;
- `crates/brassclaw_engine/src/memory/{retained_instruction,retained_inputs,retained_tools,retained_reviews,retained_validator,retained_validator_subject}.rs`;
- engine `executor/{retained_preload,retained_source,retained_recipe}.rs`;
- `crates/brassclaw_skills/src/{component_revision,revision_store,association_contract,association_review_store,pure_component_contract,review_selection_store}.rs`;
- actual retained-selection format parser/restoration modules found by `rg`,
  including database constraints and serialized nested selections. Do not infer
  file names or assume there is one reader because one Rust type is shared.

Inventory every constructor/parser/checker by supported selection version and
approval profile. At minimum include active catalogue prepare/publish/reload,
matching and task capture, ordinary execution, private validator and prefix jobs,
explicit children, waits/retained restoration, qualified MCP discovery/call,
deployment preview/activation and supported recovery. Identify historical-only
and unsupported consumers explicitly. Find strict allowed-field lists, hash
comparisons, format constants, fixtures, cross-crate reexports and SQL checks.

**Exit:** a checked inventory; every selected program has one named approval
verification owner and one immutable selection/restore owner. No duplicate
per-product approval verifier is proposed.

## 3. Define exact approval carriage before touching readers

- [ ] **B2-02 — Extend the existing retained format deliberately.**

Current `/1` selections have eight required fields and existing optional
`association_approvals`/`model_input_binding`. Preserve their exact historical
bytes, checksum domains, absence semantics and actual supported approvals.
Do not append an unchecked field and hope old strict parsers ignore it.

Implement a strict successor in the existing selection owner for selections
needing new own-function/pure/qualification references. Use an explicitly new
selection format version, with an explicit compatible reader of old `/1`.
Do not create a second persistent BuildInstruction or manifest store.

Before implementing serializers, write and review the exact successor schema
and its typed representation with B1. The following information is mandatory;
these are requirements for the **new contract**, not fields accepted by old APIs:

1. Preserve Recipe/variant/workflow-class, exact `step_link`/step order, input and
   invocation layouts, full component revisions and any selected model-input
   policy. Pin the actual selected export, not array position or default today.
2. Associate each executable occurrence with the applicable verified approval
   profile and immutable ID/checksum. Distinguish independent pure own approval,
   authored functional approval, exact shipped usage and supported legacy exact
   usage. A supported existing canonical installation qualification keeps its
   exact narrow scope; it is not the general pure exception.
3. The task graph binds exact root UUID/class/version/checksum, selected public
   export/signature/interface, current selected dependency closure and native
   artifact/binding contract. Approval separately authenticates its own original
   reviewed subject. S/legacy exact usage requires combination equality; successor
   A requires exact own roots plus qualified compatibility, not equality with all
   historical review-context dependencies. One selected graph has one revision
   per UUID; equal names/schemas never substitute for own approval or selection.
4. Retain applicable workflow approval and validator/check-set qualification
   identities through their existing owners. A Recipe approval does not approve
   constituent code, and a pure helper's dependency approval does not authorize
   executing it independently.
5. Reuse B1's verified persisted reference contract. Include the approval record's
   checksum in the selection; an ID alone cannot identify its bytes. Resolve
   evidence/provenance only through B1's checked owner.
6. Key per-step pins by the selected canonical step ID and selected export;
   reject missing, extra, duplicate, wrong-step, wrong-variant and ambiguous
   references. Declare exact schemas and supported profile tags; reject unknowns.
7. Preserve representation of historic `model_input_binding`: absence and
   explicit Default remain distinct. Approval migration cannot silently choose
   a new model/profile/prefix or convert an old task to a different route.
8. Serialize deterministically through the existing canonical byte owner. Parse
   with duplicate-key/unknown-field/data-bound checks and exact checksum/readback.
   Capture limits according to the acknowledged preparation policy; never truncate.

Update nested `component-validator-selection`/catalogue formats explicitly when
they carry this successor. Keep old `/1` readers for valid old profiles; extend
V150/later DB checks with successor migrations, never edit applied migrations.
Only write the new form when the selected contract requires it. Do not rewrite
old stored selections merely to normalize them into a newer format.

**Exit / B2-SEL contract:** one strict versioned selection implementation used by
all readers. Fixture tests prove old byte identity and new exact round trips.

### Concrete successor layout and per-root approval references

Use `monty-retained-recipe-selection/2` for the proposed successor after B2-01
confirms that version is unused. If intervening work already allocated it, reuse
that actual compatible owner contract or allocate the next explicitly versioned
successor once; do not create two meanings for `/2`.

Keep the eight existing base fields with their exact current field schemas. Keep
existing optional `association_approvals` and `model_input_binding` with their
actual meanings. Add one required nonempty `functional_approvals` object for
the new profile, keyed by exact executable step IDs. A `/1` record cannot contain
this field; a new profile cannot omit it. This is a **proposed strict successor
retained-selection contract**, not a new Recipe/association field or an API
already implemented in current source. Complete the corresponding B1 writer and
all readers before emitting it. Each occurrence value has exactly:

| Field | Proposed successor type and meaning |
| --- | --- |
| `profile` | Enum `pure_own` or `authored_function`; verified by B1, never chosen as authority by a caller |
| `implementation` | Exact `{uuid, class_code, version, checksum}` class-22 reference for the invoked code; positive integer version excluding booleans |
| `export` | Exact declared selected public export; valid nonreserved supported Python identifier |
| `interface_checksum` | SHA-256 of the exact canonical interface record defined by the B1/B2 shared owner, not a hash of mutable prose |
| `own_approvals` | Nonempty collection of exact per-root approval references defined below; no manufactured combination approval |

Each own-approval reference has exactly `root` (the actual four-field component
revision reference), `approval_id` (canonical non-nil UUID), `approval_checksum`
(64 lowercase hex SHA-256 of exact approval bytes) and `subject_checksum` (same
representation for the B1 original subject). Keep one unambiguous applicable
own record per selected root; coordinated changed roots retain separate physical
IDs and their coordinated decision linkage from B1. An unchanged partner/root
keeps its existing approval; including it as context does not reapprove it.

Resolve every required selected provider's own evidence, including applicable
Tool/ToolSkill/helper roots, through B1's same owner. The complete manifest/verified
graph carries prerequisite coverage once; repeated occurrences reuse exact
references and do not create per-invocation approval. A shared functional decision
may coordinate roots but is not a new extra combination approval. Do not assume
one `approval_id` implies that every root or dependency was approved.

B1 authenticates each record's original root/partner/dependency review context.
B2 compares the actual selected interfaces and required/provided contracts and
coherent publication basis. Preserve those distinct exact graphs. A dependency
successor may differ from the historical A context only through the verified
compatibility rule below. Changed own roots still need their own complete review.
For `pure_own`, validate exact exported pure root coverage; for
`authored_function`, validate the actual Skill/code technical association and
joint function subject without forcing an unchanged partner into a new review.

All selected executable steps requiring this profile have an entry; bound legacy/S
usages keep their exact association pins and applicable verified S owner. Do not
copy an old exact association record into `own_approvals` or accept ambiguous,
extra, wrong-root, wrong-class or wrong-profile records. A code revision with
multiple exports still uses the actual qualified invocation selection; pins do
not permit first-export guessing or multiple ambiguous usage associations.

For successor authored function steps, replace the old blanket
`association_approvals.len() == bindings.len()` assumption with exact coverage
by the disjoint applicable pin sets. Each bound occurrence resolves exactly one
supported authored-functional approval set or legacy/S exact usage approval;
every ordinary pure occurrence resolves its applicable pure approval. Continue requiring the technical
Skill/ToolSkill/Tool association and live kernel policy for authored-functional
steps. Removing the old approval-count assumption must not remove binding,
artifact, identity or dispatch checks. Old `/1` retains its original invariant.

The existing canonical-generation exception remains exact and does not require
manufactured new pure approval records. New ordinary pure/functional subjects use
the new carriage. Define the canonical interface bytes once from the strict
parsed interface/signature/contracts/dependencies; preserve their exact original
bytes or the explicitly versioned canonical representation, never independently
reconstruct different digests in separate readers. Add golden positive/negative
fixtures for every field, profile, version and optional-field presence rule.

## 4. Load exact pure approval and dependency coverage

- [ ] **B2-03 — Replace UUID-only qualification and unify the check.**

**Implemented narrow prerequisite, 2026-11-05:** A-02a (`9c8d3c12a`) removed
UUID-only lookup and uses the same exact canonical owner check for preparation
and retained verification. It separates exact occurrence refs from rooted graph
coverage without changing `/1` bytes or adding positive general approval.
Native canonical prepare/recheck/reload, foreign occurrence/version/checksum,
ordinary publication, retained child and full component-deployment regressions
passed in the [current scoped repair record](docs/validation/2026-11-03-bob-repair-progress.md).
Final twelve-package strict lint passes. The general own-approval producer and
successor carriage below are still required; do not recreate the removed UUID-only helper.

1. Keep the removed UUID-only `load_independently_qualified` absent. Extend the
   current exact shared occurrence checker with B1's genuinely verified general
   profile, supplying actual selected revision/export references. Load exact root
   version/checksum, supported approval bytes/checksum and complete closure
   through B1's trusted checker in the same coherent read.
2. Do not trust `root_id` membership, existence of any approval row, copied
   `validation_mode`, or mere FK validity. A valid v1 record never approves v2.
3. Separate two meanings in the existing catalogue model: dependency coverage
   within a qualified usage, and independent executable occurrence coverage.
   They may share verified graph facts but are not interchangeable grants.
4. Add each verified pure root and the actual selected, independently approved/
   compatibility-checked dependencies to the appropriate graph-coverage
   calculation. S/legacy exact qualification uses its exact approved closure;
   successor A does not substitute its historical review context for the new
   selected task graph. Check every pure executable occurrence
   independently, even if its UUID is already a dependency of a bound usage.
   Merely extending `independently_qualified` while leaving `covered` unchanged
   still rejects legitimate pure-only steps at the final dependency gate.
5. Extend the one checked helper already shared by preparation and
   selected-program verification. Preserve its existing exact canonical proof;
   add the general verified profile at both entries together. Never teach
   preparation about general pure approvals while retained verification still
   accepts only canonical generation exceptions.
6. Check actual selected membership/export and exact dependency edges, not a
   recipe-wide UUID union or approval for another variant. Pure source must meet
   its qualified no-host-calls/effect-free preload profile.
7. Read approvals in a bounded batch within one transaction rather than one
   query per step. Deduplicate identical exact references, never distinct
   revisions/profiles. Compare returned count/identity sets so missing rows fail.
   Do not use a mutable process-global UUID-approved cache. Any reused cache is
   keyed by exact immutable references/profile and coherent generation.

**Exit:** valid standalone pure steps prepare; helper-only or old-version
approvals fail; the same result holds during actual retained verification.

## 5. Connect shipped usages and qualified validator selection

- [ ] **B2-04 — Consume real S qualification without weakening authored checks.**

1. B1-05 supplies exact packaged subject and observed qualification, not merely
   rows in `reborn_skill_association_approvals`. Validate that ownership before
   exposing any new L001–L101 usage or structural-validator graph.
2. Keep legacy authored exact association verification unchanged for its actual
   supported profile, including Q1/behavior/Q2. Do not change its mode check to
   accept arbitrary caller-selected `system_seed`.
3. Extend the existing bootstrap/canonical qualification selection path to use
   B1's verified S receipt and complete selected closure. Remove special-case
   fallthrough that treats every non-Reply/Generation/Compiler/Encoding usage as
   authored. Do not grow a second hardcoded list of UUIDs considered trusted.
4. Compare Skill, PythonCode, ToolSkill, Tool and all transitive references to
   the exact approved usage. Resolve registered adapter/artifact/ABI; all metadata
   references must correspond to the actual retained implementation loaded.
5. Independently qualified Tool/ToolSkill prerequisites remain prerequisites;
   usage closure membership does not approve them retroactively. Preserve
   per-revision provenance for mixed A/S graphs and authored overrides.
6. Give B1-07 one qualified-validator selection operation through the existing
   catalogue/installation owner. It returns one coherent exact Recipe/variant/
   checks/artifacts/qualification snapshot. It never follows allocation heads
   independently or uses presence checks as proof of full class qualification.
7. Prepare and restore that private validator selection with the same strict
   reader and approval validation. Before global public producers start, ensure
   required private qualification is available or expose actual failed readiness.
8. Keep class validators private. No public command Recipe, raw Python MCP call,
   fake Skill or per-call component/approval is created to gain execution access.

**Exit:** actual new shipped usage and qualified private validator selections can
be consumed by the ordinary native/global path without a human action or bypass.

## 6. Publish through one coherent catalogue transaction

- [ ] **B2-05 — Connect preparation, compatibility and activation.**

1. Use `component_deployment` and the current catalogue store for read-only
   preparation and explicit activation. Match and assembly select from the same
   coherent active approved generation; no independent latest lookup afterward.
2. Preparation checks root/variant/step links, all approval references, full
   dependency/artifact closure, typed dataflow, export/binding compatibility,
   actual runner support, model-input contract and workflow approval.
3. Keep draft and unapproved revisions outside matching. If an active graph is
   corrupt or unsupported, report the repairable error; never hide it, select an
   older graph silently or reinterpret the failure as No-Match/Tier 2.
4. For A compatible successors, use B1's own-function approval and implement the
   missing directed contract checks in the existing publication/binding owners;
   do not assume those readers already exist. Preserve contracts needed
   by valid consumers for new use, including transitive exports, required inputs,
   missing/null/default distinctions, strict result shapes and effects/retries.
   Unchanged consumers keep their approvals. Do not resurrect blanket authored
   combination reapproval as the successor implementation.
   Specifically inspect `retained_tools`' current code-input/result versus
   association-schema equality checks. Retain exact equality in legacy/S profiles
   where required; successor A uses qualified required/provided variance instead:
   every Recipe/usage-supplied input must be accepted by the implementation and
   every declared implementation result must satisfy its consumers, recursively.
   Cover null/missing/defaults, strict extras, numeric/string bounds, transitive
   exports, fixed selectors, effects/errors and retry guarantees. Do not relax
   all profiles, use prose `meaning` as proof or claim arbitrary Python semantic
   equivalence. Reuse old consumer behavior cases plus applicable semantic review
   without changing/reapproving unchanged consumer revisions. Publish only when
   that complete selected profile is implemented and accepted.
5. For S, retain the exact newly qualified usage combination. A contract-compatible
   change alone cannot transfer old S evidence to new selected bytes.
6. Lock/compare the expected catalogue generation and changed component bases in
   the existing transaction/CAS. Revalidate concurrently changed consumers before
   committing the coherent new selection. Partial coordinated Skill/code approval
   cannot publish half the changed subject.
7. Commit catalogue/routing and durable affected-prefix invalidation with the
   existing supported transaction participants. The Prefix owner then builds
   from that active snapshot. Missing prefix readiness blocks only dependent
   model work, not unrelated qualified model-free tasks.
8. Keep immutable old generations, approvals and actual native/worker artifacts
   while selected tasks/checkpoints/native owners need them. Do not make current
   activation or replacement revoke an original pinned task implicitly.
9. Lost commit/reload/HTTP acknowledgement reconciles the original activation ID
   and exact bytes. Expose desired/effective/pending state. Do not create a second
   activation or claim runtime uptake from DB success alone.

**Exit:** one active generation is published atomically, old task selections stay
valid, and no unsupported profile reaches matching before its readers exist.

## 7. Connect every retained consumer before enabling the profile

- [ ] **B2-06 — Update selection serialization/restoration and ordinary capture.**

1. Update the actual engine selection constructor/parser, selected-program
   verifier, catalogue serializer/reloader, retained review preparation reader
   and DB format checks identified in B2-01. Keep parsing version-dispatched and
   closed; no broad unknown-field acceptance as a compatibility shortcut.
2. Restore original component/interface/export/approval/artifact references.
   Verify the exact selection checksum before preparing executable definitions.
   Rebuilding from current active revisions is not restoration.
3. Ordinary capture binds original message/conversation/turn/run/attempt IDs and
   the original coherent selected workflow before effects. A prepared manifest
   is not a Tool grant; kernel policy/technical limits stay live on dispatch.
4. Replace every duplicate pure-coverage check with the shared exact checker.
   Missing or conflicting pins fail before effects, including after reload.
5. Retain the actual original worker/native artifacts. A changed mutable path
   under the same name cannot satisfy old artifact identity. Preserve existing
   `PackagedExecutableImage`/native retention owners instead of copying another
   interpreter per product or task.

- [ ] **B2-07 — Update children, waits, private jobs and supported recovery.**

1. Trace each explicit child admission: retain original parent lineage/claims,
   selected child workflow/input layout, exact approvals and immutable artifacts.
   No rematching/re-resolving latest components in an admitted child.
2. Wait/resume preserves the selected graph, export and task-owned typed data;
   unrelated tasks/attempts remain isolated. Completed task state can be freed
   after its result custody is transferred; code/artifact retention is separate.
3. Validator/internal prefix jobs use their own captured original definition,
   subject and approval pins through the same private admission/restore contract.
   Neither a request string nor `trusted_internal_turn` alone certifies a graph.
4. B3's recovered original work can restore only supported retained continuations
   and actual settled results. Do not add automatic crash replay or reset Tool
   dispatch counts. Unknown outcomes retain their custody and explicit status.
5. Keep fatal generation fencing and the one-global-service rule. A profile
   reader error cannot start another VM, Rust loop or per-chat fallback.
6. If a previously unsupported reader is necessary, implement its actual owner
   and qualification before publishing the profile. Do not mark it covered by
   an inventory entry or skip the consumer's acceptance case.

- [ ] **B2-08 — Update qualified MCP and Prefix consumers.**

For MCP, use the same exact checker in command qualification, discovery refresh,
advertised contract capture and the ordinary chat task. Refresh qualified
discovery after startup/restart/catalogue changes; in-flight calls retain their
advertised selection. `tools/list` comes from qualified MCP-call Recipes, not raw
Skill/pure rows. The completed listed command opens an ordinary chat and forwards
its correlated result; no direct Monty/IBS/Rust execution or per-call Q1/Q2.
If an advertised command changes or is withdrawn before ordinary task capture,
reject the stale contract before effects; do not rematch to its new meaning.
After verified capture, the original task retains its pins. Keep the server
running; Kohai advertises/connects after final prefix addition immediately before
provider send and disconnects only that request on complete answer or reconciled
failure/cancel. Streaming progress is not completion; concurrent calls keep
their own advertised contract and lifecycle. Reuse the existing gateway owner.

For Prefix, consume the current approved catalogue snapshot and qualified export/
knowledge inventory through its existing owner. A public export is not automatic
MCP publication. Approval repair does not change prompt construction: Kohai
formats the dynamic orchestrator prompt with the accepted pair design, optional
Sempai returns the final dynamic prompt using its own target prefix/design, then
Kohai inserts the preformatted target prefix unchanged exactly once. No provider
server files, template override or new formatting pipeline is introduced.

Verify final-request and live-policy guards remain effective. A prefix package or
design for a previous graph is not silently used to cover a changed graph.

## 8. Upgrade and invalid historical record handling

- [ ] **B2-09 — Migrate without rewriting evidence or stranding valid old tasks.**

1. Inventory actually affected records/generations and determine their original
   profile. Classify valid historical supported selections separately from
   placeholder/forged/incomplete/unverifiable records; do not invalidate all old
   records merely because a new format exists.
2. Add only successor migrations, strict parsers and necessary integrity indexes.
   Do not edit V102/V104/V150/V200–V202 or silently relax their checks. Inspect
   current maximum migration and coordinate the one new number before editing.
3. Retain original immutable bytes and references. B1 requalifies real packaged
   successors with fresh exact identities. Preserve authored overrides and old
   actual artifacts; do not relabel a class-50 row or turn it into a Recipe.
4. New admissions require a valid complete active graph. If a published graph
   depends on invalid evidence, report the exact affected readiness/workflow
   failure and arrange a checked successor activation. Never silently unpublish,
   downgrade, blanket-grandfather, fall through to Tier 2 or assert approval
   because it previously ran.
5. An already selected invalid/unverifiable graph receives explicit retained
   failure/quarantine handling through its existing custody owner before new
   effects. Preserve prior effects, receipts, unresolved outcomes and original
   selection for diagnosis/reconciliation. Do not rematch/replay the task.
6. Valid old selected tasks continue on old exact references despite a compatible
   successor, withdrawal or application restart under supported recovery.
7. Rollback changes only future selection to a genuinely qualified supported
   generation. It cannot rewrite an admitted task or resurrect unsupported old
   code/artifacts. Verify restore, readback and prefix/discovery status after an
   upgrade with authored overrides and both old/new task selections present.

### Boot integrity, operator repair and retirement — absorbed missing4 Steps 11/12

These are required B2-09 deliverables; B1-05/B1-06 supply trusted package/evidence
qualification and B3-09 supplies the visible repair/review state.

1. Trace `component_boot`, `content_integrity`, the immutable revision store and
   the actual `brassclaw repair` command. A checksum check of legacy Skill/
   ToolSkill/PythonCode prose rows does not cover validator Recipe revisions.
   Verify all eight selected validator workflows and complete executable closure:
   exact stored revision bytes/checksums, layouts, bindings, imports/exports,
   code dependencies, approved check-set and retained native/worker artifacts.
   Use the revision owner's exact digest rules, not independently reserialized
   JSON with a potentially different checksum.
2. Required corrupted/missing production qualification prevents the affected
   readiness before public work starts, with safe diagnostics. Neither reseeding
   nor setting `validation_status='validated'` establishes integrity/approval.
   Inspect actual immutable reads before adding another integrity query; reuse
   a complete existing check and implement only missing closure/artifact checks.
3. Fresh boot, repeated boot and upgrade are idempotent on exact package bytes.
   Preserve deliberate operator overrides and valid old task graphs. New tasks
   choose the qualified coherent successor; never fall back to mutable rows
   when a qualified selection or approval is missing.
4. An unsealed/incomplete legacy review cannot silently adopt newly seeded code
   or its evidence. Retain its original pending records and expose that it needs
   review/supersession. A valid already sealed/admitted old review retains its
   exact original selection; a new seed does not by itself invalidate that work.
5. Repair must use the same verified package/revision/qualification/activation
   owners. Provide a real supported repair path for validator integrity faults
   and exercise it. Never copy the legacy `ON CONFLICT DO UPDATE` pattern onto
   immutable approved revisions/evidence, overwrite authored edits, fabricate a
   content edit to escape a conflict or replace an old task's artifact. Original
   corruption/unresolved custody remains preserved while checked repair and
   successor publication establish new readiness.
6. Inventory actual remaining legacy callers and retained owners before retiring
   compatibility code. Do not recreate or remove an already retired Q1 entry a
   second time. Search for `executor/code_audit.rs` and its registrations/callers;
   if already absent, record that fact. If present, retire the obsolete Rust
   audit workflow only after supported replacements and this plan's native gates
   pass and no active/retained owner needs it. Preserve needed historical bytes.
7. Acceptance includes fresh/existing/repeated boot, override-preserving upgrade,
   integrity fault -> supported repair -> checked readiness, interrupted review/
   commit, exact restore, old waiting tasks and no invented successful verdicts.
   The old plan's claims that seed qualification or catalogue integration were
   already complete are not acceptance evidence.

## 9. Acceptance matrix, build package and completion

- [ ] **B2-10 — Prove connected behavior, then update status.**

| Scenario | Required outcome |
| --- | --- |
| Pure root with genuine exact own approval, no Tool-usage dependency membership | Prepares, publishes and executes through ordinary global Monty |
| Same pure UUID/export reused by two genuinely distinct Recipes | Both selections retain identical code/approval references; actual zero-model behavior when eligible |
| Only dependency membership, another variant's approval, or v1 approval for selected v2 | Fails independent execution coverage before effects |
| Unchanged A root with an independently approved compatible dependency successor | Own approval and unchanged consumer revisions survive; technical compatibility and new task graph are checked exactly |
| Coordinated Skill/code change versus unchanged partner context | Separate changed-root own approvals commit coherently; no extra combination review or approval of context |
| Compatible A input/output variance versus existing legacy equality | New qualified profile accepts only sound directed contracts; legacy/S exact checks remain intact |
| Wrong export/interface/dependency/approval checksum, omitted/extra pin | Strict rejection in prepare and every applicable restore/capture path |
| Approved shipped Tool usage outside the four old canonical special cases | Actual S checker consumes complete exact qualification without human Q2 |
| Caller writes a seed label or submits positive JSON | No verified selection/approval or Tool permission results |
| Draft validator or concurrent component activation | New work captures one qualified coherent graph; existing work stays pinned |
| Publish/reload ordinary task then MCP command qualification | Same valid pure/usage pins survive every reader |
| Child/wait plus successor activation | Original task/child inputs, exports, approvals and artifacts remain original |
| Live Tool block after binding/wait | Actual dispatch blocked despite valid immutable approval |
| Lost activation acknowledgement, prefix invalidation or discovery wake | Original operation readback; no duplicate activation/effect or false uptake |
| Old `/1` valid record and new successor round trips | Old exact bytes/checksums unchanged; both strict supported readers work |
| Historical invalid evidence and unresolved task | Explicit scoped failure/custody preservation; no hidden fallback or replay |
| M4-06 mutable invocation isolation | Concurrent/repeated tasks/attempts share no mutable defaults/arrays/results/closures or host context |
| M4-08 deterministic preload | Actual dependency-first order is stable; missing helper/cycle/conflict/undeclared import/effectful initializer rejects |
| Boot/repair/retirement gate from B2-09 | Integrity and supported repair cover exact revision/closure/worker bytes, preserve overrides/old pins and retire only unused legacy code |

Use existing real PG/global worker/ordinary caller harnesses and the relevant
`runtime/tests/authored_catalogue_tests.rs`, deployment, MCP, child, validator
and retained-selection tests. Add tests where the actual consumer is missing;
do not count parser-only fixtures or raw DB insertion as trusted qualification.
Use the v3.5 bounded-preview/filter/formatter/reply reference and a genuine second
log-inspection workflow for actual pure reuse. Their meaning follows
[path_to_v3.5.md](path_to_v3.5.md#qualified-reference-recipe-contract); a successful
bounded preview must not claim whole-file coverage.

Review the full connected diff before a consolidated affected-package build.
Follow the development policy's two-package-chat/supervisor workflow. Package
chats use the shared serialized, activated Modal builder; the supervisor alone
runs native Cargo and tests in the inherited canonical target, with long jobs
detached in Screen. Native and Modal builds may run concurrently, each preserving
its own captured inputs. Reuse unchanged accepted evidence and compiled artifacts.
No inline target assignment or explicit shell wrapper, routine `cargo clean`,
second local target or simultaneous Cargo within one build environment.
Capture source/patch hashes, exact flags, worker/artifact/migration/settings
identities, nonempty test counts and native traces under the development policy.
No source-tree copies in validation evidence. Repair warnings rather than hide them.

Completion requires both trusted B1 production and every applicable checked
reader, with real activation/reuse/retained acceptance. A new table/query or one
successful preparation cannot close this plan. Update the existing owning plans,
implementation status and changelog with exact evidence and remaining scope.
V35-03/04 consume this work; V35-13/14's dynamic occurrences and durable recovery
remain their explicit owners. Do not relax the flat runner's occurrence fencing
or introduce a Rust workflow to make the reference pass.

## 10. Architecture/source audit — 2026-11-03

Audited after Bob 1 against the four component guides, v3.5 roadmap and the
current retained instruction, binding, review-selection and publication readers.
The successor schema is explicitly proposed implementation work; current `/1`
parsers do not already accept it. No new profile may be activated from this text.

| Finding in the earlier plan | Corrected instruction |
| --- | --- |
| A single approval ID per occurrence could collapse coordinated own roots or treat context as newly approved | B2-02 carries exact per-root `own_approvals` and checks applicable prerequisite evidence without an extra combination approval |
| Historical A dependency closure could be mistaken for every future selected graph | B2-02/B2-03 distinguish exact review provenance, actual task graph and checked A compatibility; S/legacy exact equality remains |
| Compatibility was delegated to an assumed existing reader despite `retained_tools`' exact schema equality | B2-05 names that source gate and requires actual profile-specific directed input/output checks and native regression cases |
| MCP advertised selection wording omitted stale-before-capture rejection and request connection timing | B2-08 specifies rejection before capture, original pins afterward and the existing always-on-server/per-request connection lifecycle |

All step IDs remain stable. This audit establishes corrected instructions and
source reasoning, not implementation, deployment, pure approval or full-v3.5
completion. Revalidate affected current inputs before starting source edits.
