# missing4_v3_at_k3 — Validator Steps 7–12: class routes, behavioral evidence, human Q2, production cutover

**Date:** 2026-11-02.
**Status:** Plan only. No code, runtime, seed or activation change results from
creating this file. Implementation belongs to its named owning layer. Existing
approvals, running tasks and current placeholder bytes remain unchanged until
the steps below are completed and accepted.

---

## 1. Problem statement

Steps 1–6 of `validator_v3.md` are structurally closed or operationally complete:

| Step | Status |
|------|--------|
| 1 — Freeze contracts / inventory | Closed (structural) |
| 2 — Immutable submissions / review subjects | Closed (BUILD-018, 2026-10-31) |
| 3 — Retain validator selection / trusted evidence contracts | Closed (source) |
| 4 — Repair shared seed defect | Closed |
| 5 — Reusable facts and checking components | Partial; core Prerequisites met |
| 5a — Qualify effect-free preload / pinned function invocation | Source-closed; live execution pending (missing3 ITEM-C) |
| **6** — Assemble and execute multi-step validation Recipes | **Closed**; legacy path removed; T2–T5 pending live execution (missing3_v3_at_k3) |

**Steps 7–12 are entirely open.** The current structural scaffolding
(immutable `ComponentRevisionDraft` envelopes, boot-time `qualify_all` for
L013–L101 association approvals, structural `check_structural_presence` body
shared across all eight classes) is *preparation*, not completion.

Four concrete gaps drive every open step:

| Gap | Layer | Consequence |
|-----|-------|-------------|
| **G-1 Shallow PC bodies** | `builtin_bootstrap.rs` / `brassclaw_skills::validator_components` | All eight `validator-class-N` Recipes share a single three-field structural check. Step 7 requires deep, class-specific checking logic per route. |
| **G-2 No behavioral evidence rows** | `reborn_component_review_evidence` | `bootstrap_l013_approval` writes Q1 + behavioral evidence for L013–L101 component *pairs*. No parallel mechanism writes behavioral evidence for the eight *validator* Recipes themselves or their PythonCode bodies. |
| **G-3 No human Q2 path for authored changes** | `pg_association_review`, WebUI | The existing association-review panel covers one Tool-usage pair. It does not cover class-21 (Recipe) or class-22 (PythonCode-only) review subjects, nor does it connect `pure-component-approval/1` for pure steps. |
| **G-4 No production activation / cutover** | `authored_monty_catalogue`, catalogue publication | No generation switch from the current "validated" legacy rows to a coherent immutable-revision catalogue generation has been performed. |

---

## 2. Exact current state of the infrastructure

### 2.1 What exists (re-used for this work)

- **Eight legacy `validator-class-N` Recipes** in `reborn_recipes` with
  `validation_status = 'validated'` and `consumer_tags = ["05:validator"]`.
  These are mutable rows; they are *not* the retained-path revisions.

- **Eight `ComponentRevisionDraft` envelopes** in `reborn_component_revisions`,
  one per class, produced by
  `brassclaw_skills::validator_components::structural_validator_recipe_drafts`.
  Each envelope carries a `validator_coverage` field used by
  `RetainedValidatorCoverage::from_inputs()`. These are structural scaffolding —
  they have no behavioral evidence or approval rows.

- **`boot-time qualify_all`** (`bootstrap_l013_approval::qualify_all`) writes
  `skill-association-approval/1` and `pure-component-approval/1` records for
  L013–L101 component pairs. It does *not* touch the validator Recipes themselves.

- **`run_q1_validation_retained`** — the sole production Q1 entry point, wired
  via `ValidatorTaskDriver` → `GlobalMontyDriver::drive_admitted`. Legacy path
  removed (PKG-H). T2–T5 live execution pending (missing3_v3_at_k3 ITEM-C).

- **`pg_retained_q1.rs`** — structural observation writer.

- **`pg_validator_q1_evidence.rs`** — durable Q1 evidence writer, links completed
  validator Recipe executions to `reborn_component_review_evidence` rows atomically.

- **`pg_association_review.rs`** — human Q2 route for one Tool-usage pair (one
  Skill + PythonCode). Existing scope: `catalogue_activated = false`.

- **`pure-component-review.md` exact remaining cutover contract** — eight-item
  requirement list for Steps 3/8–10. Items 1–8 are all open.

### 2.2 What does NOT exist (required by Steps 7–12)

- Class-specific PythonCode bodies for the eight validator routes (G-1).
- Behavioral evidence rows for each validator Recipe's own execution (G-2).
- Q1 + behavioral evidence for the eight validator PythonCode components (G-2).
- `pure-component-approval/1` records for the validator's pure-logic PythonCode (G-2).
- `skill-association-approval/1` records for Tool-calling validator steps (G-2).
- Human Q2 support for class-21 (Recipe) and class-22 (PythonCode) subjects (G-3).
- Coherent catalogue generation covering the immutable validator revisions (G-4).
- Production cutover: `authored_monty_catalogue` selecting from immutable
  revisions instead of legacy mutable rows for the validator paths (G-4).

---

## 3. Step-by-step implementation plan

The steps below follow `validator_v3.md` §4 Steps 7–12 and the
`pure-component-review.md` exact remaining cutover contract. Each step has a
bounded deliverable and an explicit acceptance gate. No step activates against
incomplete prerequisites.

---

### Step 7-A — Author class-specific PythonCode bodies (one per route)

**Owner:** `crates/brassclaw_skills/src/validator_components.rs` and
`crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`.

The current shared `PC_VALIDATOR_STRUCTURAL_BODY` checks only that `name`,
`description` and `content`/`steps` are non-empty. Step 7 requires each class
route to check its own class-relevant properties.

For each class route, write a dedicated `def check_class_N(inputs):` body that
performs **all** the checks listed in `validator_v3.md` §Step 7 for that class.
The inputs arrive as typed JSON string data (`inputs["candidate_json"]`); the
body decodes, inspects and returns `{"pass": bool, "errors": [str]}`.

Class-specific required checks (summary — read `validator_v3.md` §Step 7 for
the full requirement per route):

| Class | Key structural checks beyond name/desc/content presence |
|-------|----------------------------------------------------------|
| 0 (Tool) | Declared operation/effect contract; registered callable schema; retained artifact/checksum; capability UUID mapping |
| 1–3 (Skill) | Prose sections; strict checksummed interface; `skill-association/1` present with class-correct UUID relationships; callable compatibility; recursive return/failure contract; zero-Tool vs one-Tool distinction |
| 13 (ToolSkill) | Tool UUID/callable/adapter agreement; registered parameter schema vs binding; fixed selectors; no executable/permission content |
| 21 (Recipe) | Actual persisted steps/variants/links; ordinals/classes/one UUID cardinality; matching binding+export pairs; typed capture/input mappings; tier restrictions |
| 22 (PythonCode) | Parse source under Monty profile; distinguish preloadable defs from invocations; exports vs load-time effects; zero or one host call per step; no forbidden identifiers |
| 23 (ExtensionCatalogue) | Domain overview + actual task-group/Recipe inventory; correct target classes; child IDs/dependency closure |

**Implementation rules:**

1. Each body is a pure-logic function with `import json` (declared dependency)
   and no `host.*` calls. Inputs are `inputs["candidate_json"]` (string).
2. Use `json.loads(inputs["candidate_json"])` to decode the candidate.
3. Return `{"pass": bool, "errors": [str]}`.
4. Reject unknown fields, nil/wrong-class UUIDs, cycles and incompatible types
   where applicable. Keep deterministic sections free of LLM calls.
5. Each body is stored as a separate constant in `validator_components.rs`
   (e.g., `CLASS_0_STRUCTURAL_BODY`, `CLASS_1_STRUCTURAL_BODY`, …).

**Pass the class-specific body to `structural_validator_recipe_drafts`** instead
of the current shared `PC_VALIDATOR_STRUCTURAL_BODY`:

```rust
// In builtin_bootstrap.rs retain_validator_check_drafts():
let class_bodies: &[(i16, &str, &str)] = &[
    (0,  "Tool",                CLASS_0_STRUCTURAL_BODY),
    (1,  "leaf Skill",          CLASS_1_STRUCTURAL_BODY),
    (2,  "domain Skill",        CLASS_2_STRUCTURAL_BODY),
    (3,  "LLM Skill",           CLASS_3_STRUCTURAL_BODY),
    (13, "ToolSkill",           CLASS_13_STRUCTURAL_BODY),
    (21, "Recipe",              CLASS_21_STRUCTURAL_BODY),
    (22, "PythonCode",          CLASS_22_STRUCTURAL_BODY),
    (23, "ExtensionCatalogue",  CLASS_23_STRUCTURAL_BODY),
];
for (class_code, class_label, pc_body) in class_bodies {
    let drafts = structural_validator_recipe_drafts(*class_code, class_label, pc_body)?;
    …
}
```

**Acceptance:**
- Every class route's `ComponentRevisionDraft` carries a distinct, non-trivial
  `pc_body` that addresses its class-specific requirements from `validator_v3.md §Step 7`.
- `cargo test -p brassclaw_skills structural_validator_recipe`: all pass.
- `cargo clippy -p brassclaw_skills --all-targets -- -D warnings`: 0 warnings.

---

### Step 7-B — Negative fixture tests per class route

**Owner:** `crates/brassclaw_skills/src/validator_components.rs` test module,
or a new `crates/brassclaw_reborn_composition/src/runtime/tests/validator_class_route_tests.rs`.

For each class, add at least two negative fixture tests:

1. **Valid input:** a minimal well-formed candidate for that class returns `{"pass": true, "errors": []}`.
2. **Invalid input:** a known class-specific defect (e.g., missing `skill-association/1`
   for class-1; multiple UUIDs in one `include` for class-21; a `host.tool()` call
   at module level for class-22) returns `{"pass": false, "errors": [<non-empty>]}`.

These tests run the body through the Monty interpreter using the existing
`brassclaw_monty_host` test infrastructure to confirm behavior, not only parsing.

**Acceptance:**
- `cargo test -p brassclaw_skills validator_class`: at least 16 cases (2 per class), all pass.

---

### Step 8 — Complete behavioral evidence for each validator Recipe

**Owner:** New `crates/brassclaw_reborn_composition/src/bootstrap_validator_approval.rs`
(parallel to `bootstrap_l013_approval.rs`).

For each of the eight `validator-class-N` Recipes and their paired PythonCode,
produce behavioral evidence rows using the same `bootstrap_evidence` pattern
used in `bootstrap_l013_approval.rs`:

1. Read the exact `ComponentRevisionDraft` from `reborn_component_revisions`
   (produced in Steps 7-A above).
2. Build a `"component-review-evidence/1"` Q1 evidence record referencing the
   exact PC/Recipe revision checksums.
3. Run a bounded behavioral observation through the existing packaged
   `PackagedExecutableImage` worker:
   - Supply the minimal valid candidate JSON for that class code and confirm `pass=true`.
   - Supply the negative defect fixture and confirm `pass=false, errors[...]`.
   - Record actual observed `pass`, `errors` list and the worker task handle.
4. Write the behavioral evidence record (`"kind": "behavior"`) with
   `"validation_mode": "system_seed"` using `bootstrap_evidence::retain_completed`.
5. For the validator's own PythonCode (pure-logic, no host calls): write a
   `"pure-component-approval/1"` record via
   `pg_pure_component_approval::record_pure_component_approval`.

All UUIDs are deterministic (UUID v5 from the component bytes + a fixed
`VALIDATOR_QUAL_NAMESPACE` constant) for idempotent re-runs.

**Refer to `pure-component-review.md` items 3–5** (observe required checks,
retain the actual interpreter, finish both trusted approval paths) before
claiming behavioral evidence is complete.

**Acceptance:**
- `SELECT count(*) FROM reborn_component_review_evidence WHERE kind='behavior'
  AND component_id IN (<8 validator PC UUIDs>)` returns 8 after boot.
- `SELECT count(*) FROM reborn_pure_component_approvals WHERE root IN (<8 validator PC UUIDs>)`
  returns 8 after boot.
- `run_content_integrity_check` does not fail for any validator Recipe or PC row.

---

### Step 9 — Human Q2 path for authored (non-seed) validator changes

**Owner:** `crates/brassclaw_reborn_composition/src/pg_association_review.rs`,
`crates/brassclaw_product_workflow/src/settings.rs`,
`crates/brassclaw_webui_v2_static/…/settings-tabs.js`.

Step 9 of `validator_v3.md` requires that *installed authored changes* to
validator Recipes or their PythonCode go through authenticated human Q2.
The existing association-review panel covers only one Tool-usage (class-1 Skill)
pair. It must be extended to cover:

- **Class-21 (Recipe):** The recipe-specific review route shows the draft
  revision, exact validator selection, Q1/behavioral check results and requires
  an authenticated operator action to write an immutable approval record.
- **Class-22 (PythonCode-only):** The pure-logic review route shows source,
  export declarations, dependency closure and behavioral observations; requires
  the same authenticated action.
- **Multi-class review:** A single WebUI view that accepts all component classes
  in the `submitted_for_review` state, shows the exact evidence set (not a
  recomputed latest view), and refuses to emit an approval if any required
  evidence record is missing or its checksum mismatches.

**Exact boundaries from `validator_v3.md` §Step 9:**
- Human review verifies semantics, coverage and the selected Q1/behavior records.
- The immutable human evidence and association approvals are written through
  trusted writers using `skills.md` formats unchanged.
- A literal `human` field is not proof of authenticated action; use the existing
  `simplified Phase 1` ingress.
- Authored Q2 applies only to installed changes; shipped packages use Step 11's
  `system_seed` path with `q2_ref = null`.

**This step does not extend to the shipped (system_seed) validator components.**
The seed path (Steps 7-A/8) already sets `q2_ref = null` per the bootstrap contract.

**Required work:**
1. Extend `pg_association_review.rs` to accept class-21 and class-22 subjects
   in addition to the existing class-1 usage.
2. Add the `human-review/class-21` and `human-review/class-22` API routes to
   `reborn_services.rs` and the WebUI.
3. Show the reviewed combination (draft revision + evidence records), not a
   recomputed view from latest mutable rows.
4. Gate the approval write on: Q1 evidence exists and checksum matches, at least
   one behavioral evidence record exists and checksum matches, no partial/missing
   required checks.
5. Write the immutable human approval record using the existing
   `bootstrap_evidence::retain_completed` path (or its successor for non-seed Q2).

**Acceptance:**
- A test authored Recipe change (submitted via WebUI) goes through:
  `submitted → Q1 pass → behavioral evidence → human Q2 view → approved → next task selection`.
- The approval record is immutable; a later body edit creates a new submission,
  not a mutation of the approved row.
- A fake human marker in the API body is rejected.

---

### Step 10 — Coherent catalogue activation for validator revisions

**Owner:** `authored_monty_catalogue.rs`, `pg_component_revision_store.rs`,
catalogue publication path (`simplified_v3.md §8.2`).

Currently the catalogue uses the legacy mutable `reborn_recipes` rows for
validator routing (`find_validator_recipe`). Step 10 prepares the switch to
the immutable revision path:

1. **Supply review/support predicates** for each class route to the catalogue
   prepare path (`authored_monty_catalogue::prepare_with_policy`):
   - A class-N component has an active approval record if and only if
     `reborn_pure_component_approvals` (for pure) or
     `reborn_skill_association_approvals` (for Tool-calling) has a matching row
     with checksum agreement against the pinned revision.
   - The independent-occurrence gate (`require_independent_execution_coverage`)
     must succeed for every unbound executable step in the validator Recipe variant
     before the generation is published.

2. **Extend the catalogue generation** to include the eight validator Recipe
   revisions when all their approval records are present.

3. **Prevalidate consuming combinations** before publication: a changed validator
   PC or Recipe affects all class routes. Keep incompatible combinations in draft.

4. **Follow `simplified_v3.md §8.2` exactly**: publish the fully approved
   executable catalogue together with durable affected-prefix invalidation, then
   let the prefix owner build from that active snapshot. Do not require a Prefix
   produced from an unpublished candidate.

5. **Update matching, named lookup, retrieval, IBS** to use the active-generation
   references consistently. Preserve internal `05:validator` routing tags while
   excluding those routes from ordinary user matching.

**Refer to `pure-component-review.md` items 6–7** (extend the independent
occurrence gate with real approvals; carry approval pins through every reader).

**Acceptance gate from `validator_v3.md §Step 10`:**
> Concurrent activation yields one complete old/new generation; an old waiting
> task continues its original code, while a live Tool block affects its next dispatch.

---

### Step 11 — Bootstrap, migration and upgrade

**Owner:** `component_boot.rs`, `builtin_bootstrap.rs`, `content_integrity.rs`,
and the new `bootstrap_validator_approval.rs` (from Step 8).

1. Extend `run_content_integrity_check` to also verify the eight validator
   Recipe `ComponentRevisionDraft` documents: compute the SHA-256 of each
   draft's canonical JSON bytes and compare against the stored checksum. A
   mismatch aborts boot.

2. Provide idempotent upgrade mapping: if a running instance has the legacy
   mutable `validator-class-N` rows (from `seed_validator_recipes`) and new
   immutable draft revisions are present (from `structural_validator_recipe_drafts`),
   the upgrade path in `bootstrap_validator_approval::qualify_all_validators`
   must detect the existing legacy rows, retain the new immutable revisions
   idempotently and never overwrite an operator override.

3. Preserve old artifacts and evidence. Legacy `reborn_recipes` rows for the
   eight validators remain readable for any task that selected them; only
   newly admitted tasks select the new generation.

4. Ensure the `brassclaw repair` command re-seeds the validator revisions when
   their content checksums mismatch (per the existing `ON CONFLICT DO UPDATE`
   pattern in `repair.rs`).

**Acceptance gate from `validator_v3.md §Step 11`:**
> fresh/existing boot, repeated boot, upgrade with overrides, interrupted
> review/commit and restore preserve trust and task retention without invented passes.

---

### Step 12 — WebUI/API and production cutover

**Owner:** WebUI `settings-tabs.js`, `pg_settings_listing.rs`,
`crates/brassclaw_product_workflow/src/settings.rs`,
`crates/brassclaw_reborn_webui_ingress/…`.

1. **Show class-route detail in the WebUI component catalog:**
   - For each `validator-class-N` Recipe in the Settings → Component Catalog tab,
     display: draft revision, active revision (if any), exact validator selection,
     check failures, missing evidence, behavioral observations and human Q2 status.
   - Show the *reviewed combination*, not a display recomputed from latest mutable rows.

2. **Preserve pending/failed/incomplete/rejected states** and resubmission:
   disabling approval when backend gates are incomplete; the UI is not the gate.

3. **Support operator editing** of validator Recipes and associated PythonCode
   through immutable versions and the same Q1/behavior/Q2 contract. Do not expose
   success-evidence writes or mutable approved-body editing.

4. **Production acceptance matrix** (from `validator_v3.md §5`, abridged):
   - All eight class routes: valid candidate reaches required gates; distinct
     invalid candidate is rejected for its class defect.
   - Metadata/source separation: hostile strings remain data; checksum unchanged.
   - Full Recipe inspection: empty steps, bad links, multiple UUIDs, unpaired
     bindings and invalid result edges fail.
   - Human/provenance: fake human flags, old receipts and self-issued evidence fail.
   - Authored production end-to-end: actual installed submission → pinned validator
     exports → evidence → human Q2 → publication → next matched task works.
   - Bundled production end-to-end: reviewed installation package → noncircular
     integrity/Q1/behavior → system_seed records with null Q2 → coherent publication.

5. **Replace the legacy Q1 entry path** only after the above acceptance matrix
   is complete. The legacy `seed_validator_recipes` `validation_status='validated'`
   path is already bypassed by `run_q1_validation_retained`; no second removal
   is required, but the legacy mutable rows must remain readable until all
   running tasks that may have selected them have completed.

6. Update `docs/implementation-status.md` with a dated entry recording:
   - Commits for each step (7-A through 12).
   - Confirmation that all eight class routes have class-specific PC bodies,
     behavioral evidence records and (for system-seed components) `pure-component-approval/1`
     rows in `reborn_pure_component_approvals`.
   - Confirmation that the human Q2 path for authored changes covers classes 21 and 22.
   - Confirmation that the production acceptance matrix cases passed.

---

## 4. Dependency and ordering constraints

```
Step 7-A  (class-specific PC bodies)
    │
    ├──► Step 7-B  (negative fixture tests — independent, can run in parallel)
    │
    ▼
Step 8    (behavioral evidence for validator Recipes — requires final PC bodies)
    │
    ├──► Step 9    (human Q2 path — independent of Step 8's seed path)
    │
    ▼
Step 10   (coherent catalogue activation — requires Step 8 approvals present)
    │
    ▼
Step 11   (bootstrap/migration — requires Step 10's activation contract)
    │
    ▼
Step 12   (WebUI/production cutover — requires all prior gates)
```

Steps 7-B and Step 9 can be developed in parallel with Steps 7-A and 8
respectively, but **cannot activate** before their prerequisites are complete.

Steps 7–12 also depend on two already-open external items:

- **missing3_v3_at_k3 ITEM-C** (T2–T5 live execution): provides the behavioral
  receipt for `run_q1_validation_retained` that Steps 8/9/12 will rely on.
- **simplified_v3.md Phase 3a** (durable recovery, resource accounts): Steps 9
  and 12 use the shared admission/recovery contract; missing support blocks
  affected review work without blocking unrelated deterministic routes.

---

## 5. What this plan does NOT change

- **L013–L101 component qualification** — covered by `missing2_v3_at_k3.md`
  and `bootstrap_l013_approval.rs`. Not touched here.
- **missing3_v3_at_k3 ITEM-C** (T2–T5 live execution) — tracked separately.
  These validator-infrastructure integration tests are prerequisites, not
  deliverables of this plan.
- **General standalone pure approval** — `pure-component-review.md` items 1–8
  describe the complete cutover contract. This plan implements the subset
  required for the eight validator class routes; the general activation path
  for arbitrary operator-authored pure components (composition R10, V35-03/04)
  remains separate open work.
- **MCP publication** — having behavioral evidence and approval records for
  validator Recipes does not publish them as MCP commands. They are internal
  private routes (`05:validator` tag) and must remain excluded from ordinary
  user matching.
- **prefix Phase C2/C3/D** — Step 10's catalogue publication triggers prefix
  invalidation; the prefix *owner* builds from the active snapshot. This plan
  does not implement prefix generation or serving.
- **Dynamic occurrences (V35-13/14)** — conditional/repeated check coverage in
  Steps 7–8 requires the occurrence/control owner to supply qualified control
  primitives. Until those are available, validator workflows that require
  conditional branching over class-specific check sub-sets remain as drafts.
- **Classes 10/50 (Orchestrator/Scaffold) roots** — Step 7's requirement note
  states these must not be accidentally certified by a generic class-1/2/3
  route. Their coordinated review is an explicit dependency; no generic validator
  route may certify them.

---

## 6. Constraints inherited from the owning architecture

- **`system_seed` approval uses `q2_ref = null`.** Per `AGENTS.md §Installation
  seeds and post-installation changes`, first-party packaged validators use the
  controlled seed path with `q2_ref = null`. An installed operator edit requires
  the authored Q1/behavior/human-Q2 path (Steps 9/12).

- **Association is embedded in the revision, not a standalone row.** The
  `"association"` field of the `"component-revision/1"` envelope must be embedded
  in `reborn_component_revisions` (same pattern as `missing2_v3_at_k3.md` §6).

- **One UUID per component step.** `validator-class-N` Recipes must follow the
  one-UUID-per-`include` rule; the current structural scaffold already does this.
  Class-specific bodies must not add multi-UUID steps.

- **No candidate-source execution.** Candidate JSON is string data decoded by
  `json.loads()`; the validator body must never `eval()` or `exec()` candidate source.

- **`retain_completed` is the only legitimate approval writer** (atomically writes
  behavioral evidence + approval row). The preceding `retain_intent` for Q1 uses
  `ON CONFLICT DO NOTHING` and is idempotent.

- **All approval UUIDs are deterministic (UUID v5)** derived from component bytes
  + a fixed namespace constant. Re-runs produce no duplicate inserts.

- **`run_q1_validation_retained` is the sole production Q1 entry point.**
  No second Q1 path, validator-specific scheduler or Rust sweep loop is introduced.

- **Draft containment.** Draft validator Recipe revisions are assembled and
  tested only in the constrained review environment. Ordinary matching sees only
  the coherent active approved catalogue (`05:validator`-tagged rows excluded
  from user matching in all cases).

---

## 7. Review corrections record

This plan was authored against live source. No prior version exists; this is the
first draft. If corrections are required after source review, record them here with
the same format used in `missing2_v3_at_k3.md §7`.
