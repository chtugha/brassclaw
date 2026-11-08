# Repair Bob 1 — trusted approval and Q1 at K3

Date: 2026-11-03. Status: **implementation plan; no repair is accepted by this document**.
Reviewed committed baseline: `958cbb9c86d50644c59fcd40ced9c5a7cafc9cdc`.
Review comparison: `33e49b32554df2a1004cfc65832c8af3047650a3..958cbb9c86d50644c59fcd40ced9c5a7cafc9cdc`.

## 1. Scope, precedence and single implementation ownership

This plan repairs the trusted producers of Q1, behavior, human Q2 and shipped
qualification. It is a bounded execution breakdown of the existing Validator
and installation owners, not another approval architecture or queue. Follow
[AGENTS.md](AGENTS.md), [development policy](docs/development-policy.md),
[recipe.md](recipe.md), [skills.md](skills.md), [tools.md](tools.md) and
[toolskills.md](toolskills.md). The binding own-functional A / exact-usage S
distinction in skills.md governs over historical combination-only examples.

Existing implementation owners remain [validator_v3.md](validator_v3.md),
and [pure review](docs/reborn/pure-component-review.md#exact-remaining-cutover-contract).
[missing4_v3_at_k3.md](missing4_v3_at_k3.md) is now a superseded navigation and
history reference; its remaining work is absorbed into the named Bob steps.
Do not implement its historical instructions as a parallel checklist. Consume
already corrected source/evidence from the existing owners without recreating
their implementation. Record repair progress here and cross-reference accepted
results from the normative owner. Supersession is not implementation completion.

| Responsibility | Sole repair owner | Consumers |
| --- | --- | --- |
| Exact review subjects, required-rule coverage, observed verdicts, trusted evidence and approval production | This plan, B1 | B2 and B3 |
| Exact approved graph selection, approval pins, publication and every retained reader | [Plan 2](Repair_bob_2_v3_at_k3.md), B2 | B1 validator admission, B3 task hosting |
| Durable execution/wakes/claims, shared driver lifecycle and native Codex integration | [Plan 3](Repair_bob_3_v3_at_k3.md), B3 | B1 review jobs, ordinary product consumers |

Recipes/PythonCode select and aggregate checks. Rust authenticates subjects,
native facts, claims, artifacts and writes. Neither Rust nor an LLM may fabricate
successful reports. ResultOnly completion, source inspection, successful parsing
and a caller's `succeeded` flag are not approval. Only an actual No-Match enters
Tier 2; validator failures never use that route. Keep one global Monty service.

### Required order across all three plans

1. Reconcile the three current source maps first. B1-02/B1-03 define original
   subjects and reusable checking Recipes; B1-04 defines the observed versus
   terminally settled verdict boundary. B2-02 designs carriage with those owners,
   and B3-06/B3-08 design durable work and settlement against the same contract.
2. Complete B1-05/B1-06 and B2-03/B2-04 as connected producer/reader packages.
   Installation qualification uses its existing noncircular native/contained
   boundary, never a normal task whose own unqualified graph is its prerequisite.
   Verify each prerequisite before a dependent qualification can consume it.
3. Connect B1-07/B1-08 with B3-06/B3-07/B3-08: qualified original request ->
   actual Monty observations -> terminal native settlement -> exact verdict and
   graduation. No callback waits for a terminal receipt that depends on itself.
4. Complete B2-02/B2-05 through B2-09, including every serializer, restoration,
   child/private/MCP consumer, compatibility rule and migration, before enabling
   publication of the new profile. Preparation tests may run in the controlled
   integration environment earlier; they do not authorize partial live cutover.
5. Validate each completed coherent package under B1-09 through B1-11 and the
   affected B2/B3 gates. Run connected final publication/product cases only once
   their prerequisites exist. B3 Codex work may proceed independently in its own
   existing provider/Auth owner; no placeholder implementation bridges a gap.

These are dependency stages, not permission to defer a required safety check.
No stage calls a later unimplemented API or substitutes a permissive stub.

### Mandatory implementation and evidence quality

All review workflows are Recipes of reusable approved usages and small pure
components, assembled by IBS and sequenced by Monty. Record actual reused UUIDs,
exports, typed edges, private entry provenance and terminal owner before source
work. Rust may authenticate observations or apply one transaction; it must not
grow a validator-specific rule loop, behavioral workflow Tool or model decider.

Do not ship stubs, `unimplemented!`, successful no-op branches, synthetic receipts,
or simulated dispatch as a repair. Negative test data and isolated test resources
are legitimate inputs, but acceptance executes the actual PG stores, native
host/kernel adapters, retained artifacts and global task path. An in-memory fake
or injected success object cannot establish those claims. Real issuer/model
compatibility needs its own real-service evidence; a test endpoint cannot certify it.

Use existing acknowledged settings for runtime limits, admission/backpressure,
timeouts and whole-value bounds. Add a necessary setting through its actual
DB/API/WebUI/live-ACK owner, with units, explanation and safe feasibility rules,
rather than embed a new validator-only cap, case quota, retry count or deadline.
Representation bounds and the current supported stop/one-dispatch contract are
technical contracts, not arbitrary constants to replace with guessed values.

### Review-finding coverage and handoffs

| Review finding | Exact repair steps | Downstream gate |
| --- | --- | --- |
| Placeholder bytes and unperformed successful seed checks | B1-03, B1-05, B1-06 | B2-04 actual qualification consumption |
| Public approval JSON/borrowed evidence/false seed provenance | B1-02, B1-04, B1-06 | B2-03 checked exact reference |
| Seed identities omit revision and skip successors | B1-05 items 8–10, B1-06 item 11 | B2-09 historical repair |
| Different queue candidate and immutable subject | B1-02, B1-08 | B3-06 atomic original-work binding |
| Completed negative validator treated as passed | B1-04, B1-08 | B3-07 actual verdict projection |
| Allocation heads choose validator drafts | B1-07 | B2-04 qualified selector |
| Input/result/receipt/preload contracts disagree | B1-03, B1-09/B1-10 | B2-06 strict restore and B3-08 hosting |

## 2. Source baseline and reconciliation before edits

- [x] **B1-01 — Reconcile the exact current source and active work.**

**Completed source reconciliation — 2026-11-03:** the
[committed-source map](docs/validation/2026-11-03-bob-committed-reconciliation.md)
rebases this plan on `0f1236a6c`, preserves the other agent's committed work and
records which defects remain. This closes inventory only, not runtime gates.

Read the affected crate guides and record HEAD plus before-hashes for affected
paths. At the reviewed baseline the shared worktree already contained edits in
`bootstrap_validation.rs`, `bootstrap_validator_approval.rs`,
`builtin_bootstrap.rs`, `global_monty_driver.rs`, `runtime.rs`, three retained
validator test modules and `brassclaw_skills/src/validator_components.rs`.
The subsequent audit also observed an active `global_recipe_ports.rs` repair;
reconcile that actual diff before touching receipt/finish behavior.
`missing4_v3_at_k3.md` also contained revised instructions. Re-read their actual
pre-consolidation text in its linked archive when checking prior work; its old
instructions no longer govern implementation. Re-read current source diffs and
preserve the author's changes. A working-tree repair is neither missing
code to recreate nor accepted evidence. Rebase each step against current source.

Inspect these existing producers and consumers, not similarly named legacy paths:

- `crates/brassclaw_reborn_composition/src/{bootstrap_l013_approval,bootstrap_validator_approval,bootstrap_evidence,component_boot,builtin_bootstrap}.rs`;
- composition `{q1_orchestrator,pg_review_admission_source,pg_validator_q1_admission,pg_validator_q1_evidence,validation_review,validation_queue}.rs`;
- composition `{pg_pure_component_approval,pg_pure_approval_store,pg_retained_q1,pg_workflow_review,pg_association_review,pg_review_submission}.rs`;
- `crates/brassclaw_skills/src/{component_revision,revision_store,review_submission_store,review_selection_store,review_function_subject,pure_component_contract,association_review_store,validator_components}.rs` and the referenced validator Python files;
- `crates/brassclaw_engine/src/memory/{retained_validator,retained_validator_subject}.rs` and actual retained executor observation types;
- V102, V104, V150 and V200–V202 migrations, plus all later migrations present
  when implementation starts. Allocate any successor after that inventory.

Record which review findings still reproduce in source. In particular check
placeholder qualification, unconditional success writes, duplicate constructors
for the same validator UUID, draft-head selection, subject/queue mismatch and
completion-versus-verdict confusion. Do not mark a finding fixed just because
its original failing line was removed; trace its downstream replacement.

**Exit:** one affected-path/interface map and a reconciled list of remaining
defects, attached to the implementation record. No source reset or blanket clean.

## 3. Seal the exact review request

- [ ] **B1-02 — Bind a queue candidate to its original immutable submission.**

**Integrated bounded prerequisite, 2026-11-05:** B-01 through `668158a5c`
adds exact immutable original/queue binding, V206, complete supported projections,
readback/CAS and visible unresolved legacy/joint states. Actual original capture,
changed-candidate/concurrent-author/ACK-loss, full paging/metadata/escaped-size,
retained Q1 and Sempai draft cases pass. All 24 UI-state/paging cases pass.
The corrected lifecycle cases and final twelve-package strict lint pass;
full `/3` execution, legacy custody disposition and durable original Work remain
open. See [the exact current repair scope](docs/validation/2026-11-03-bob-repair-progress.md).
The complete B1-02 exit below is not claimed.

1. Extend the existing review submission/queue transaction boundary to retain
   the exact submission identity and checksum with the queued candidate. This
   semantic binding belongs here; durable execution metadata belongs to B3-06.
2. Capture stable component UUID, class, exact candidate version/checksum,
   comparison base, complete selected dependencies and the actual proposed
   payload. Missing versus null and the original exact document bytes matter.
3. For installed A changes, use the existing own-function subject machinery.
   Inspect `review_function_subject.rs`: Skill and implementing PythonCode are
   one logical functional review with separate physical identities. Bind changed
   roots and unchanged partner context; context is not another approval candidate.
4. If current single-root `/1` preparation rejects a joint function subject,
   implement its strict, explicitly versioned successor in the same owner.
   Update all necessary writers/readers with B2; do not drop the partner or
   relabel a joint subject as old `/1`. Coordinated changed roots approve together.
5. Stop looking up `ORDER BY recorded_at DESC LIMIT 1` to choose which submission
   to validate. Production scheduling must supply the retained submission ID.
   A compatibility caller with only component UUID must resolve its explicit
   queue binding, never a latest submission unrelated to the queue proposal.
6. Lock/compare the existing queue and component in their established lock order.
   Retain submission and binding atomically in the same PG owner transaction
   where possible. Do not hold the transaction while preparing/executing Monty.
7. Final graduation compares the original subject and exact candidate binding,
   queue state, bases and existing CAS bytes again. A valid result for A cannot
   graduate B, even if B was captured earlier and remained unchanged.
8. Same operation ID and identical bytes returns the original binding. Reusing
   the ID for a different candidate, partner, base or dependency is a conflict.
9. Define legacy pending-row handling with B3-06: retain original provably
   unstarted work or show an explicit unresolved/resubmission state. Never
   attach an arbitrary latest subject and call it the original review.

**Exit:** a sealed original review request, consumed by both scheduler and Q1;
the public submission DTO cannot manufacture this seal.

## 4. Use one canonical validator graph and honest coverage

- [ ] **B1-03 — Reconcile typed inputs, rule results, aggregation and receipts.**

**Already implemented, scoped:** `0f1236a6c` supplies the canonical separate
receipt encoder and its fingerprint observation, while retaining the aggregate
object and strict rule result shape. T1–T5b and twelve utility body cases are
recorded in the implementation index. Complete eight-class retained admission,
required checks and historical format compatibility remain to be accepted here;
do not repeat the former aggregate-object/string repair.

1. Keep one canonical construction of each packaged validator graph in the
   existing `validator_components`/bootstrap owner. Seeder and qualifier reuse
   exactly those draft bytes and returned references. Qualification must not
   reconstruct a different class-22 document under the same stable UUID.
2. Preserve the observer's sealed full-subject input path: task input is
   `subject_json`, a typed JSON string of the exact sealed subject. A qualified
   pure reader extracts component fields. Do not return to separate mutable
   `name/description/content` reads or source substitution.
3. Each required rule returns the current strict rule shape
   `{rule_id: string, ok: boolean, errors: list[string]}`. The rule ID belongs to
   the selected qualified check set, not the candidate. Reject contradictory
   `ok`/errors values, missing/duplicate/foreign rules and malformed diagnostics.
4. Reuse `validator_collect.py` and `validator_aggregate.py`. Collection preserves
   exact rule results/order; aggregation returns the existing typed
   `{pass: boolean, errors: list[diagnostic]}` contract. Define recursive schemas
   for every list/object; no untyped list, inferred `any` or undeclared field.
5. Fix receipt verification without changing a rule's actual result into an
   envelope string. Keep the aggregate's observed result an object. Transport
   an internal completion receipt separately through the existing ResultOnly
   completion contract, bind its result references to the sealed executor
   observations, and verify every per-step input/result fingerprint. If encoding
   changes, version that receipt explicitly and retain the old parser for its
   actual supported scope. Never require the same observed aggregate to be both
   a JSON string and an object.
6. Declare actual imports, private symbols, constants, exports, default export
   and dependencies. For example `import json as _json` must agree with the
   supported preload declaration. Inspect the actual Monty language restrictions.
7. Keep all class routes private, without public intent/MCP exposure or fake
   chat messages. No Tool binding is invented for a pure checking step.
8. A preparation or transport error remains an infrastructure/incomplete outcome;
   a mechanically valid negative check remains a completed negative verdict.

**Exit:** each of classes 0, 1, 2, 3, 13, 21, 22 and 23 prepares, executes and
produces a verifiable receipt through the real retained admission path. This
alone establishes structural execution, not complete class Q1 or behavior.

- [ ] **B1-04 — Implement the required checks and a sealed verdict.**

**Implemented narrow transport boundary:** B-00 now authenticates the exact
observed verdict only after native terminal settlement; cancellation/incomplete
execution cannot become a negative candidate verdict. The incomplete-execution
regression, all six retained Q1 callers and exact diagnostics/Oracle cases passed
in the latest [scoped acceptance](docs/validation/2026-11-03-bob-repair-progress.md),
with affected strict lint. Reuse that proof/reader. Required complete class rules,
genuine behavior and trusted approval production below remain open; this does
not close B1-04 or general own approval.

Build the required-rule manifest in the existing validator owner; bind its
revision/checksum to the selected validator graph. A candidate may supply data,
never its required rule list, passing expectations or empty behavior matrix.

| Reviewed subject | Required coverage to implement/reuse before whole-subject Q1 |
| --- | --- |
| Tool, class 0 | Stable identity, registered capability/callable, actual retained native artifact and adapter/ABI, recursive parameters/results, technical/effect/failure contract and independently observed primitive behavior |
| ToolSkill, class 13 | Exact Tool identity, compatible registered adapter/callable, parameter and fixed-selector compatibility, result/error meaning, no execution or permission in metadata |
| Skill, classes 1–3, with implementing code | Own usage/interface/export/signature, exact association, reachable actual host targets and approved prerequisites, input/computed argument/result/failure compatibility, prose/code semantic review and behavior; do not run a second complete review of the same implementation |
| PythonCode, class 22 | Actual parsed definitions and reachable graph, declared preload symbols/imports/dependencies, effect-free load, exact recursive contracts, pure restrictions when applicable, every public export and meaningful valid/invalid/boundary behavior |
| Recipe, class 21 | Every supported variant/step_link, one UUID per component step, binding/execution pairing, approved selected exports, typed edges/order, model/tier declarations, explicit failure/completion/reply ownership and meaningful workflow behavior |
| ExtensionCatalogue, class 23 | Actual current catalogue schema and referenced components/task groups, coherent scope/overview and non-executable metadata; no inherited component approval from membership |

Reuse the source-inventory, preload-definition, recursive-contract, Python graph,
pure-call and Skill-interface helpers already in `validator_components.rs`.
Native parsing/facts are authenticated observations, not candidate execution or
semantic approval. Enumerate the remaining rule-to-export mappings and implement
the missing ones in the existing owner. Mechanical presence checks must never
stand in for this table. Human semantic review remains part of A; deterministic
Q1 does not ask an LLM to reapprove Tier-0 semantics at task startup.

The eight generic routes have an explicit class allowlist: 0, 1, 2, 3, 13, 21,
22 and 23. Class 10 is the protected Orchestrator and uses its separately owned
root-review contract, never a generic Skill route. Class 50 is historical Scaffold
storage, not a new active component type or validator to implement. Preserve its
old bytes/pins; never reclassify its UUID as class 21 to manufacture coverage.
If a required conditional/repeated check cannot be represented by the supported
flat runner, qualify an explicitly supported fixed variant or keep that workflow
unactivated pending V35-13/14. Do not remove checks or weaken occurrence fencing.

Introduce or extend opaque internal verdict types with private checked constructors.
Persisted wire records have their own strict readers; deserializing a DTO never
directly constructs the trusted observation/verdict or grants its authority.
Use two explicit stages: an original-subject-bound observation constructed from
actual complete per-step input/result facts, then a settled review verdict
constructed only after the task's terminal receipt and required native custody
acknowledgement. The latter binds original subject, validator/check-set, observed
results, task/run/attempt/service/worker and settlement identities plus the
observed pass/fail diagnostics. These are **internal contracts to implement**,
not caller-constructible JSON or a claim that an existing DTO already has them.
No successful Q1/approval/graduation follows from the first stage alone.

Distinguish passed, completed-negative, incomplete/infrastructure failure,
cancelled/stale and unresolved settlement. A completed negative result is useful
immutable evidence but cannot produce approval. For every export, behavior must
exercise the actual candidate in the supported contained, policy-enforced
environment; an example string or parser pass is not behavioral evidence.

## 5. Qualify actual shipped bytes without circular trust

- [ ] **B1-05 — Replace placeholder and self-asserted seed qualification.**

**Scoped progress, 2026-11-08:** Run-20 confirms genuine common installation
startup and five E-01 ACK/cancellation/worker-death cases on `377ad1bec`. The
originals/settings case exposed an unsupported test request (depth 65 above the
transport ceiling); its source correction `0039b365e` awaits rerun. The reviewed
F-01 checker/support/29-case Source workflow package is integrated through
`7b4c29596`, without compilation/native acceptance yet. It does not implement
general candidate Pure approval or complete class-Q1. Preserve the exact remaining
steps below and reuse these owners; do not recreate their custody infrastructure.


1. Remove placeholder prose/bodies from the qualification input. Obtain the real
   packaged immutable draft/export/association references from the seeding owner.
   Do not infer identity by a matching name in mutable legacy rows.
2. Resolve Tool UUID, capability, host callable, adapter, artifact and ABI through
   actual registration. Do not derive them by stripping/prefixing strings.
3. Declare the noncircular qualification DAG: trusted installation/runtime base
   verifies native Tool; independently qualified native fixtures verify its
   ToolSkill binding; then verify Skill/code use; finally consuming workflows.
   Independently reused prerequisites are checked for exact applicability.
4. Use the existing controlled bootstrap/contained validation facilities and
   retained worker image. Native/utility checking is not another global
   orchestrator. Establish any private installation admission before public
   producers start; do not require an unqualified dependent Skill to approve
   its own Tool or validator. Spell out boot ordering and failure cleanup.
   Separate the trusted packaged installation base from the subject being
   qualified; retain its actual application/worker/native artifacts. Before
   public admission, qualify prerequisites in dependency order with existing
   closed installation facilities. No permissive temporary root, self-approved
   candidate, second production service or new unqualified dispatcher may break
   a qualification cycle.
5. Run actual declared exports with meaningful fixtures. Use isolated temporary
   mounts, controlled network/provider fixtures and original native dispatch
   observation. Never run seed shell/mutation cases against the operator's real
   files/accounts. Tool and kernel policy remain effective in the fixture.
   Real isolated mounts and actual test-service HTTP responses exercise the
   production boundary; do not inject a fake host/provider result in its place.
   A restrictive test policy may narrow an existing allow, never override a live
   production block. Required shell/spawn_subagent workflows remain Tier 1 even
   for shipped deterministic commands. Use actual permitted technical settings.
6. Every persisted check/report must correspond to a performed check with exact
   subject, fixture, outcome, artifact and observation identity. Delete code that
   manufactures `succeeded: true` from a list of check names.
7. Tool-usage `/1` approvals include Skill, PythonCode, ToolSkill, Tool and the
   complete transitive selected graph. Pure approval includes the exact pure
   root/export/interface and dependency coverage. A usage closure is not own
   approval of every dependency or an independently executable helper.
8. Derive new evidence/approval identities from a domain-separated format version
   and exact subject/selection/artifact digests, not UUID/name/association alone.
   Preserve exact canonical bytes; test a one-byte code/contract/artifact change.
   A qualification reservation identifies the exact package/subject/check-set/
   selection/artifact claim. Bind its original actual execution ID before effects;
   derive per-execution evidence from that retained identity. Re-executing the
   same source in a genuinely new authorized review is a different execution,
   whereas lost-ack readback reuses the original. Do not hash wall time or returned
   JSON to invent a replacement identity after uncertain execution.
9. Idempotence verifies complete stored bytes and checksums. An existing identity
   with different bytes is an integrity conflict, never a successful skip.
10. Retain intent before possible effects using the existing owner. Retain
    observed return/outcome durably; approval may follow in a transaction only
    after verified observations. If a crash separates evidence from approval,
    reconcile the original records and finish that approval without executing
    the behavior again. Existing intent alone is not successful qualification.
11. Preserve authored overrides and retained old revisions. Do not convert an
    installed edit into S through `source='system'`, a mode flag or reseeding.
12. Missing required production seed bytes/registration/qualification fail the
    affected readiness explicitly. Fixture-only subsets must be explicitly
    declared by their contained test setup; blanket silent skipping is forbidden.

**Exit:** exact verified S receipts for applicable shipped subjects, with null
Q2 and no external maintainer or installed human action. B2 consumes the receipts;
storing records alone does not activate or publish anything.

## 6. Make evidence and approval trustworthy

- [ ] **B1-06 — Repair the writer, routes and persistence contract.**

1. Restrict successful approval creation to the existing trusted review/
   installation owner. `PgPureApprovalStore` must not treat public arbitrary
   approval JSON as authority. Keep operator prepare/approve/read operations
   facade-shaped; no raw PG/secret/dispatcher access in HTTP handlers.
2. An operator request identifies the exact existing review and expected view
   checksum. The owner constructs approval from verified state. If a legacy raw
   declaration endpoint remains, it may only reference/read or strictly resolve
   an already authorized exact decision; posting JSON cannot invent a successful
   review or select S provenance. Keep route/descriptor/client behavior coherent.
3. For every referenced record verify stored checksum, strict format, evidence ID,
   distinct required kind/stage, success where required, exact roots/versions/
   checksums/exports/closure, original subject/base/partner, required-check-set,
   selected validator and real observed run/attempt/worker/settlement custody.
4. Resolve the dependency graph from immutable revisions and compare its complete
   closure. Comparing only the caller-supplied component list to existing rows
   does not prove that dependencies were included or approved.
   Preserve the exact original review context in evidence. For S/legacy exact
   usage, the selected complete combination must equal the qualified record.
   For successor A, authenticate the changed roots' own review with its original
   partner/context, then consume B2's checked compatibility for a later selected
   dependency successor. Do not require the old A context graph to equal every
   future compatible task graph or reapprove unchanged consumers.
5. A human Q2 record authenticates the actual operator decision against the exact
   displayed view, candidate/bases and selected Q1/behavior evidence. An LLM,
   model port or unrelated `human_q2`-shaped row cannot supply it. Same evidence
   cannot serve multiple stages. Reject a changed view/base at approval CAS.
6. S provenance comes from the qualified packaged subject in B1-05. The caller's
   mode or source label cannot choose it. Mixed graphs keep provenance per
   actual revision/usage. A coordinated A Skill/code change has joint own review,
   no new combination approval for unchanged consumers in its successor profile.
7. Inspect actual V102/V104/V200–V202 constraints. Add a checked successor schema
   and strict parser where new subject/observation fields require it. Define all
   required/optional fields, recursive types, checksum domains and supported
   versions before writing the migration. Do not relabel expanded pure evidence
   under an unchanged legacy format or edit any applied migration.
8. Check actual V201 links: independent FKs do not prove that submission candidate
   and approval root are the same revision. Enforce their complete agreement in
   the trusted writer and, where representable, successor constraints/indexes.
   Its unique submission/component/version key also prevents a second approval
   for that same tuple. Specify supported re-review/decision identity explicitly;
   do not promise multiple approvals because a comment says they are possible.
9. Check V202 natural-key semantics: a reobserved task has different task/run/
   attempt identity. Recover the original durable execution for retries; never
   collapse distinct executions under one supposedly byte-identical evidence ID.
   Version storage if multiple legitimate execution identities must be retained.
10. Persist decision/approval/link/index coherently in one existing-owner
    transaction or an explicit durable reconciliation protocol. Unknown commit
    uses original-ID readback and exact-byte comparison. Never overwrite immutable
    evidence or rerun an external effect to obtain a second acknowledgement.
11. Audit pre-repair records without rewriting or grandfathering them. Successful
    flags, placeholder bytes or matching UUIDs do not establish validity. Preserve
    records for diagnosis, identify affected active generations/tasks, and arrange
    checked repair with B2-09. Valid historical profiles retain their meaning.

Resolve prerequisite own-approval production for Tool, ToolSkill and applicable
other changed roots through this same review owner as well. Reuse actual existing
writers where their subject/profile is correct; implement a missing typed writer
and its native cases before the reader can claim support. A Skill/code review
cannot stand in for own approval of a changed primitive/binding, and the new
authored profile cannot be implemented by issuing extra exact-combination reviews.

**Exit / B1-APP handoff:** the existing owner exposes an opaque verified approval
reference, with exact roots/exports/closure, approval ID/digest, supported profile
and trusted evidence/provenance available to B2's checker. It is not publicly
constructible from JSON and grants no Tool permission. B2 owns manifest carriage,
not another evidence verifier or approval writer.

B2 may retain original review/evidence checksums as provenance; that does not
freeze successor A to that historical dependency combination. Own approval and
compatibility are separate checked facts. Both the original evidence graph and
the actual new task graph remain exact and immutable in their respective records.

### Class-21 and class-22 Q2 dispatch — absorbed missing4 Step 9

This is mandatory B1-06 work, not another approval service or an additional step
owned by the superseded plan. B3-09 implements its presentation/client wiring.

1. Trace the actual operator review route, descriptors, `RebornServicesApi`,
   composition adapters and typed review subject. The current association
   handler is Skill-ID based; do not assume generic class-code dispatch already
   exists just because the historical plan named it. Extend the existing review
   surface/dispatcher deliberately, with all facade/descriptor/client callers.
2. Dispatch from the stored sealed submission's class/profile, not a caller's
   `class_code` or `human` marker. Reuse existing authenticated instance-operator
   ingress and its trusted human-review context. Add no tenant/feature-role or
   per-invocation permission scheme. Model/Sempai/component-management ports
   may submit unreviewed proposals but cannot supply successful human evidence.
3. Class 22 pure own review uses a distinct trusted pure-Q2 store operation.
   After exact Q1/behavior/view/CAS checks, write the applicable immutable pure
   approval and exact V201/successor submission link in the owner's transaction.
   Do not pass a class-22 UUID as a Skill ID into an association approval writer.
4. Class 21 uses a distinct trusted workflow-Q2 store operation: approve the
   Recipe's own supported workflows/variants and exact review evidence through
   the existing workflow owner. It must not write `skill-association-approval/1`
   or approve all dependencies. Define any missing strict workflow approval
   storage/reader once under B1-06/B2, with a successor migration and exact pins.
5. Skill/code functional review keeps its joint subject semantics. Preserve
   already supported legacy exact association review under its original scope;
   do not misuse it for Recipe or standalone pure Q2. Compatible unchanged
   prerequisites/consumers receive no artificial new review.
6. Prepare/read returns the original candidate/base/partner, selected validator,
   required checks, actual Q1/behavior evidence and missing/rejected gates from
   the immutable subject. A current mutable component row is not that view.
   Both component detail and queue review use the same prepared exact view.
7. Approve checks the prepared view checksum, original IDs, current expected
   bases and unchanged evidence selection server-side. Denial/rejection retains
   the exact decision/feedback through existing state. An incomplete review is
   non-approvable even if a browser enables its button or a caller posts directly.
8. Keep one review action surface keyed by the original submission; add no
   separately named per-class API families. Extend an actually missing typed
   port/descriptor once when the existing surface cannot represent this, retaining
   supported legacy clients and returning explicit unsupported-profile errors.
9. Native acceptance covers installed Recipe and pure-code edits separately:
   original submission -> selected validator execution -> genuine Q1 and behavior
   -> exact human view/decision -> immutable approval -> B2 publication -> new
   task selection. Shipped validators follow B1-05 with null Q2, not this route.

## 7. Admit qualified validators and apply their actual verdict

- [ ] **B1-07 — Replace latest-draft validator lookup.**

Use B2's coherent qualified-validator selection. Read the actual approved graph
and installation provenance in one coherent snapshot; never separately call
`read_allocation_head` to decide executable versions. Pin Recipe/variant/order,
all components/exports/contracts/dependencies, approval references and retained
worker/check-set identity with the original review work before execution.

Preserve selected validators across queue wait, restart reconciliation and
component replacement. A new draft must not change existing work. Retaining
preparation via `retain_validator_selection` supplies no approval by itself.
Extend `review_selection_store` and observer restoration using B2's format once;
do not bypass strict parsing to accept a new field or joint subject.

Capture acknowledged effective preparation limits using the existing live
settings owner. Do not call `MontyExecutionLimits::default()` for a real review
request when an effective profile exists. Capture revision and limits together;
later changes apply through their supported live boundary, never rewrite a
stored subject/selection or silently truncate it.

- [ ] **B1-08 — Persist and graduate a subject-bound verdict.**

**Already implemented, scoped:** `71d5c4f49` routes the recorded negative
structural outcome to failure instead of inferring pass from ResultOnly.
The subsequent repair replaces the latest-row verdict query with an opaque
post-terminal driver proof plus exact run/claim/VM/task/selection/subject and
durable settlement checks, preserving actual rule diagnostics. The early
18/19 batch and its V102-invalid foreign-result fixture remain historical failed
evidence. B-00's subsequent native acceptance covers the corrected original-bound
verdict/Oracle cases and all six retained Q1 callers. B-01 also supplies the
accepted bounded original queue/submission binding. Full required-check coverage,
the connected private Work path and idempotent durable graduation remain open;
this step stays unchecked. See the
[current exact receipts](docs/validation/2026-11-03-bob-repair-progress.md).

1. Inside `GlobalRecipePorts::finish`, the internal source verifies the original
   selected program, actual input/result observations and all required checks.
   `verify_receipt` runs before task terminal delivery: it may retain authenticated
   provisional observations, including negative checks, but must not await the
   terminal receipt or publish successful settled Q1 at this point. Enforce its
   constructor assumptions and release locks before later settlement operations.
2. After the actual task terminates, the existing native completion owner binds
   that receipt, original run/attempt/service and required settlement/accounting
   acknowledgement to the provisional observation. Only then may B1's trusted
   writer construct/commit the settled verdict/evidence. Missing/uncertain
   settlement stays incomplete/unresolved. ResultOnly is a completion kind,
   never `pass=true`. Keep observation and settlement persistence idempotent by
   their original IDs; do not create a second task or wait cycle to finish Q1.
3. `run_q1_validation_retained` consumes that exact verdict for the exact original
   request. Positive complete verdict graduates through `gate1_pass_reviewed`;
   complete negative verdict records its actual diagnostics through the failure
   writer; incomplete/cancelled/stale/unresolved never graduate.
4. Recheck original submission/subject/check-set binding, candidate/base/partner
   CAS and queue ownership in the graduation transaction. A stale attempt cannot
   change queue state after a successor claim or operator cancellation.
5. Use actual queue semantics: the current failure writer keeps state 1 with
   diagnostics; do not invent state 3 from old plan prose. If richer outcomes
   require a successor schema, B3 owns durable lifecycle status and all readers
   migrate together. Human Q2 stays separate from a passing automated verdict.
6. A lost acknowledgement first reads original evidence and graduation status.
   An identical reapplication is idempotent; conflicting verdict/bytes fail.
   Never reset counts, rerun behavior or replay an entire Recipe for persistence.

**Exit / B1-Q1 handoff:** B3 schedules the sealed original request, receives a
subject-bound result/status, and reconciles its original run. B3 does not infer
approval from `LoopExit::Completed`, create subjects or interpret rule meaning.

## 8. Required regression and native acceptance

- [ ] **B1-09 — Add focused producer/contract regression cases.**

| Case | Required observation |
| --- | --- |
| Real seed body differs from placeholder; mandatory host argument supplied | Retained/qualified bytes are the actual packaged implementation; real fixture dispatch uses declared arguments |
| Change own code/interface or actual implementation artifact | New changed root needs its applicable own A review or exact S qualification; old evidence never approves changed root bytes |
| Compatible dependency replacement with unchanged A root | Preserve original own approval/consumer revisions, verify compatibility at coherent publication and pin exact new task graph |
| Dependency/partner replacement in an S exact usage | New exact usage qualification required; individual approval does not transfer old combination evidence |
| Required prerequisite missing; fixture subset explicitly declared | Production readiness refuses; declared contained fixture has no production-support claim |
| Existing intent or evidence but missing approval after crash | Original observation reconciles, no behavior replay or success-shaped skip |
| Eight class routes, valid and negative candidates | All use the same complete-subject/rule/receipt contracts; actual negative diagnostic retained |
| Missing/duplicate/foreign rule, malformed result or undeclared import | Rejected before successful Q1/approval |
| Incomplete rule set or one unobserved public export | No whole-subject success |
| Candidate source contains quotes, Unicode, newlines and Python-like text | Exact typed data preserved; no source substitution |

- [ ] **B1-10 — Add real PG/global Monty integrity and interruption cases.**

Cover borrowed successful evidence, wrong kind, different candidate/base/partner/
closure/check-set/worker, changed byte checksum, forged S mode and unauthorized
Q2 production. All reject without approval or activation. Review A while queue
contains B; change the queue after capture; stage a newer validator draft; rotate
active validators during a waiting review. Prove exact rejection/retention.

Exercise completed-negative aggregation without transport failure: persist the
negative observation and keep the queue ungraduated with its real diagnostics.
Exercise cancellation, stale claims, lost result persistence and lost graduation
acknowledgements. Check native execution/dispatch counters, original run identity,
retained receipts and zero replay, rather than asserting only a returned enum.
Interrupt separately before observation retention, after observation but before
task terminal delivery, during native settlement, and after settled evidence but
before graduation ACK. No deadlock, premature successful Q1, released unresolved
custody or repeated behavior is allowed at any of these boundaries.

Accept complete authored pure and Skill/code own-function review through actual
operator Q2. Separately accept shipped integrity/behavior with null Q2 and no
extra installed action. Preserve valid old exact authored profiles. Tests with
unconnected fakes or parser-only assertions do not close these gates.

- [ ] **B1-11 — Validate a coherent package and publish honest status.**

### Full validator production gate — absorbed missing4 Step 12

The following cases are mandatory completion criteria for B1-09/B1-10 and
B1-11. They do not replace the existing negative/interruption cases above.
Where a consumer belongs to B2/B3, use its actual implementation and shared
receipt; do not write another checker or count a helper-only pass as acceptance.

| Case | Required test and outcome | Connected owner |
| --- | --- | --- |
| M4-01: all eight class routes | Each allowed class has a meaningful valid case reaching its required gates and a distinct class-specific invalid case rejected for its actual defect; a nonempty-body check alone cannot certify it | B1-03/B1-04/B1-10 |
| M4-02: source/data separation | Quotes, newlines, Unicode and source-like candidate strings remain exact typed data; executed validator code/checksum stays unchanged; candidate source is not evaluated as part of structural inspection | B1-03/B1-10 |
| M4-03: complete Recipe inspection | Empty executable steps, bad step links, multiple UUIDs, unpaired bindings, forward/invalid result edges and unsupported tier/flow fail before effects | B1-04/B2-05 |
| M4-04: complete functional Skill | Missing association, hidden independent multi-Tool usage and prose/implementation disagreement fail applicable own review/final approval; reused approved prerequisites are not reapproved | B1-04/B1-06 |
| M4-05: interface and selection integrity | Changing only an export/signature/fixed argument changes the exact reviewed interface/subject; undeclared symbols, incompatible aliases and ambiguous export selection fail; old association formats keep their meaning | B1-04/B2-02/B2-06 |
| M4-06: mutable invocation isolation | Repeated/concurrent tasks and attempts isolate mutable defaults, inputs, arrays, results and closures; no foreign input/host context leaks through shared globals/caches | B2-07/B2-10 |
| M4-07: old/new export and native-host coexistence | Activate a qualified successor during an old task's wait; each uses its original code/interface/approval/native implementation, and current Tool policy still affects its next actual dispatch | B2-05/B2-07/B2-10 |
| M4-08: deterministic preload | Dependency-first loading with the existing deterministic tie rule is repeatable; missing helpers, cycles, symbol conflicts, undeclared imports and effectful initializers fail before invocation | B1-03/B2-06/B2-10 |
| M4-09: authentic evidence/human/provenance | Fake human flags, copied old receipts, self-issued successful records, borrowed evidence and false seed labels cannot approve or activate a subject; model/component-management ports cannot write Q2 | B1-06/B1-10/B3-09 |
| M4-10: authored installed product flow | Actual operator submission of both Recipe and pure code follows pinned validator -> complete evidence -> exact human view/decision -> immutable approval -> coherent publication -> next matched ordinary task | B1-06/B1-10/B2-05/B3-09 |
| M4-11: shipped product flow | Actual reviewed package uses noncircular integrity/Q1/behavior, exact S records with null Q2 and coherent publication; no installed human or external-maintainer approval | B1-05/B2-04/B2-10 |
| M4-12: no accidental protected-root coverage | Generic routes reject class 10 and historical class 50; no dummy Scaffold validator or UUID reclassification is introduced | B1-04/B1-10 |

Keep per-case source/graph/artifact/setting identities and actual test counts.
Historical TASK-022 results, when available, establish only the cases they
actually exercised; they cannot certify repaired behavior with changed inputs.
No fixed fixture-count quota substitutes for coverage of all required rules,
exports, supported variants and the cases above.

### B1-11 validation procedure

Follow the development policy's two-package-chat/supervisor workflow. Package
chats implement in their isolated worktrees and share the serialized, activated
Modal builder; the supervisor reviews, validates locally and integrates accepted
changes. Record exact hashes and required features. Cargo is serial within each
build environment; native and Modal builds may run concurrently with their
independently frozen inputs. Run long local checks in detached Screen. No inline
`CARGO_TARGET_DIR` or explicit shell wrapper. Prose-only corrections require
Markdown/link/diff checks, not compilation.

For implementation, use affected-package unit/contract checks and the existing
composition native integration harness with `postgres,skills-db` and required
provider features. Read actual manifest features and PG test prerequisites first;
confirm nonempty test selection and no early-return/skipped-infrastructure pass.
Build the worker when actually required and reuse its exact artifact. Compile
once per coherent repair package, then run the selected cases from that build.

Retain evidence in the development-policy format: `source.json`, `applied.json`,
original runner, per-step logs, `completed.json`, applicable compiled test binaries
and separate `accepted.json`. Do not copy the source tree into validation folders.
Fix warnings in affected code instead of suppressing them. Update owner plans,
implementation status and changelog only with the actual accepted scope.

## 9. Completion and downstream v3.5 handoff

This plan is complete only when real bytes and all required checks establish
trusted evidence, exact subjects cannot be swapped, negative/completed/incomplete
outcomes remain distinct, and A/S approval paths pass their native tests. B2 must
also accept these records through publication and every applicable retained reader;
writer-only success is not full delivery. B3 must prove durable original-request
execution and reconciliation, without which automatic Q1 remains incomplete.

[path_to_v3.5.md](path_to_v3.5.md) V35-03/04 and V35-17–19 consume this owner.
They must not create a new approval store, waive pure approval, treat presence
checks as class Q1 or activate dynamic occurrences prematurely. V35-13/14 keep
their distinct repeated-occurrence/control-flow ownership. Preserve old task
pins, immutable artifacts and authored overrides throughout.

## 10. Architecture/source audit — 2026-11-03

All four component guides and the v3.5 roadmap were reread for this audit.
The committed baseline above and current uncommitted source were distinguished;
no runtime acceptance or completed repair is claimed. These corrections are
binding refinements of the existing steps, not another implementation workstream.

| Finding in the earlier plan | Corrected instruction |
| --- | --- |
| Exact review-context/closure wording could force new A combination review for compatible dependencies | B1-06 and B1-APP separate original evidence from new task selection; B1-09 has A-positive and S-exact negative cases |
| Receipt verification could wait for its own terminal completion and publish Q1 before settlement | B1-04/B1-08 now separate authenticated observations from the later terminally settled verdict; interruption acceptance covers each boundary |
| Cross-plan ordering could publish before all readers or rely on an unqualified validator to bootstrap itself | Shared dependency stages gate complete cutover and use the existing noncircular installation boundary |
| A logical functional review could be mistaken for approval of all prerequisite roots | B1-06 explicitly requires actual own Tool/ToolSkill/other-root writers and preserves coordinated physical root identities |
| Fixture/limit language did not fully exclude synthetic success or ad hoc operational caps | Mandatory quality rule and B1-05 require actual isolated native execution, effective policy, correct tier and existing configurable limits |

Before implementation, reconcile later source changes and preserve already
accepted applicable evidence. The audit changed plan instructions only; all
implementation checkboxes remain open pending their actual required evidence.
