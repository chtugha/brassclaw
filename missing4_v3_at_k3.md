# missing4_v3_at_k3 — superseded reference and implementation redirects

**Superseded on 2026-11-03 by the three Repair Bob plans.**
This file is navigation and historical context, not an implementation checklist.
Its remaining requirements have been consolidated into the steps below.
**Superseded does not mean implemented, validated, approved or activated.**

Use these active repair execution plans, with the existing architecture/semantic
owners in `validator_v3.md` and the four component guides still binding:

1. [Repair Bob 1 — trusted approval and Q1](Repair_bob_1_v3_at_k3.md).
2. [Repair Bob 2 — catalogue, publication and retained readers](Repair_bob_2_v3_at_k3.md).
3. [Repair Bob 3 — Codex and scheduling integration](Repair_bob_3_v3_at_k3.md).

The prerequisite order across all three is specified in
[Bob 1's shared dependency order](Repair_bob_1_v3_at_k3.md#required-order-across-all-three-plans).
Choose a named Bob step and reuse its existing owner/interfaces. Do not implement
this historical plan and the Bob plans as parallel workstreams. Current source,
uncommitted repairs and applicable receipts must be reconciled first.

## 1. Step-by-step redirect table

Steps 1–8 below retain their original Validator numbering for navigation. Their
historical status claims are not renewed by this consolidation. Source that
already works is reused; actual remaining defects and acceptance belong to Bob.

| Original step or prerequisite | Active implementation steps | Exact disposition |
| --- | --- | --- |
| 1 — contracts and inventory | [B1-01](Repair_bob_1_v3_at_k3.md#2-source-baseline-and-reconciliation-before-edits), [B2-01](Repair_bob_2_v3_at_k3.md#2-reconcile-actual-source-and-build-the-complete-reader-inventory), B3-01 | Reconcile current source, features, contracts, existing evidence and active changes; do not redo valid inventory mechanically |
| 2 — immutable submissions/review subjects | [B1-02](Repair_bob_1_v3_at_k3.md#3-seal-the-exact-review-request), B3-06 | Seal exact candidate/base/partner/dependency subject; bind it atomically to original durable review work |
| 3 — selection and trusted evidence | B1-04, [B1-06](Repair_bob_1_v3_at_k3.md#6-make-evidence-and-approval-trustworthy), B1-07, B2-02/B2-06 | Complete observed verdict/evidence, strict versioned references, approved coherent selection and checked restoration |
| 4 — shared seed defect | [B1-03](Repair_bob_1_v3_at_k3.md#4-use-one-canonical-validator-graph-and-honest-coverage), B1-09 | Reuse corrected source; verify one canonical preload/input/result/receipt contract through real admission |
| 5 — reusable facts/checking components | B1-03/B1-04/B1-09/B1-10 | Reuse existing parser/source/contract/graph helpers; finish actual required rule, export, binding and behavior coverage |
| 5a — effect-free preload and pinned invocation | B1-03, B1-05, B2-06/B2-10 | Actual declared exports/imports/dependencies and retained image; parsing or a source-only fixture is not approval |
| 6 — multi-step retained validator execution | B1-03/B1-07/B1-08, B3-06/B3-07/B3-08 | Original qualified selection runs through the single global service with subject-bound verdict and shared custody |
| 7-A — eight class-specific bodies | B1-03/B1-04 | Reconcile existing bodies, use complete sealed subject and actual strict rule/aggregate contracts; no false whole-class Q1 |
| 7-B — class-specific negative fixtures | B1-09/B1-10, B2-10, B3-11 | Actual native positive/negative/interruption acceptance; existing tests count only for their exact source/claim |
| 8 — behavior and shipped qualification | [B1-05](Repair_bob_1_v3_at_k3.md#5-qualify-actual-shipped-bytes-without-circular-trust), B1-06, B2-04 | Qualify real packaged bytes and independently qualified prerequisites, retain real observations/complete closure, consume verified S records |
| G-5 — evidence-schema successor | B1-06, [B2-02](Repair_bob_2_v3_at_k3.md#3-define-exact-approval-carriage-before-touching-readers) | Inventory V102/V104/V150/V200–V202 and later migrations; define strict linked successor formats without rewriting applied migrations or relabeling legacy evidence |
| TASK-022 — T2–T5 and fixture retest | B1-09/B1-10/B1-11, B3-11/B3-12 | Preserve actual historical receipts; run applicable changed-source cases centrally, with nonempty native tests and exact artifacts; no automatic closed status |
| 9 — authored human Q2 | [B1-06](Repair_bob_1_v3_at_k3.md#6-make-evidence-and-approval-trustworthy), [B3-09](Repair_bob_3_v3_at_k3.md#7-finish-the-product-projections-and-portable-build-configuration) | Class-21 workflow and class-22 pure review have distinct trusted store operations behind the existing operator review surface; exact-view/CAS, authentic human evidence and immutable links |
| 10-A — private validator selection | [B1-07](Repair_bob_1_v3_at_k3.md#7-admit-qualified-validators-and-apply-their-actual-verdict), [B2-04](Repair_bob_2_v3_at_k3.md#5-connect-shipped-usages-and-qualified-validator-selection) | Select one exact qualified private graph; no allocation-head selection or mutable-row fallback for new work; retain valid old task selections |
| 10-B — catalogue/publication/readers | B2-02 through B2-08 | Exact revision/export approvals, complete graph/independent occurrence coverage, coherent publication and every ordinary/private/child/wait/MCP/Prefix reader |
| 11.1 — boot integrity | [B2-09](Repair_bob_2_v3_at_k3.md#8-upgrade-and-invalid-historical-record-handling), B1-05 | Verify selected validator revisions and complete layout/code/approval/native-worker closure through actual digest/artifact owners |
| 11.2 — upgrade/overrides/pending reviews | B2-09, B1-02/B1-06, B3-06/B3-09 | Idempotent qualified successors preserve overrides, original pending records and valid sealed old work; incomplete legacy work never silently adopts new code |
| 11.3 — retain old artifacts/evidence | B2-06/B2-07/B2-09, B3-08 | Retain original code/approval/artifacts/custody while actual old tasks or unresolved native owners need them |
| 11.4 — operator repair | B2-09, B3-09 | Supported verified repair path; no mutable upsert of approved immutable revisions/evidence, no forced override or fabricated success |
| 12.1 — validator catalogue/detail UI | B3-09 | Draft versus active/effective revision, exact subject/validator/check set, failures/missing evidence/observations and Q2/provenance shown from original records |
| 12.2 — pending/failed/incomplete/rejected state | B3-06/B3-07/B3-09, B1-08 | Durable safe projections and real backend gates; distinguish correction/new submission from original-work reconciliation |
| 12.3 — immutable validator editing | B3-09, B1-02/B1-06, B2-05/B2-09 | New draft and predecessor/base CAS; preserve approved/active originals and old task pins; no success-evidence editor |
| 12.4 — full production acceptance | [B1-09/B1-10/B1-11](Repair_bob_1_v3_at_k3.md#8-required-regression-and-native-acceptance), B2-10, B3-11/B3-12 | Mandatory M4-01–M4-12 matrix plus native interruption/consumer gates; actual authored and shipped product paths |
| 12.5 — retire legacy Q1/audit paths | B2-09, B3-08 | Inventory actual remaining code/registrations/retained owners, including historical code_audit reference; retire only when replacement acceptance and retention permit; do not repeat already completed removals |
| 12.6 — status/evidence reporting | B1-11, B2-10, B3-12 | Dated exact commits/receipts, class-21/class-22 human-review coverage, immutable private selection and full matrix results; unexercised gates stay open |

## 2. Pure-review cutover contract redirects

The [eight-item pure-review contract](docs/reborn/pure-component-review.md#exact-remaining-cutover-contract)
retains its semantic ownership under Validator Steps 3/8–10. It is implemented
once through the following Bob steps; V35-03/04 and Composition R10 consume it.
This is shared support for applicable pure subjects, not a validator-only second
approval mechanism or a blanket grant to arbitrary code.

| Pure-review item | Active repair steps |
| --- | --- |
| 1 — trusted review owner, linked strict storage/schema | B1-02/B1-06, B2-02 |
| 2 — original subject versus validator/check identity | B1-02/B1-04/B1-07/B1-08, B3-06 |
| 3 — required checks and every actual export | B1-03/B1-04/B1-09/B1-10 |
| 4 — retained actual interpreter and execution custody | B1-04/B1-05, B2-06/B2-07, B3-08 |
| 5 — authored and shipped trusted approval paths | B1-05/B1-06, B3-09 |
| 6 — exact independent occurrence approval | B2-03/B2-04/B2-10 |
| 7 — approval pins through every reader | B2-02/B2-05/B2-06/B2-07/B2-08 |
| 8 — real publication and reuse acceptance | B1-10, B2-10, B3-09/B3-11/B3-12 |

## 3. Corrections that replace historical instructions

- The historical claim that Step 8 S qualification was already correct/closed is
  not retained. B1-05/B1-06 require actual bytes, performed checks, exact identity,
  independent prerequisites and observed behavior; B2-04 proves consumption.
  Genuine shipped S still has null Q2 and requires no installed/external human
  action. Installed edits follow their own authenticated A review.
- The UUID-only pure-approval query is not sufficient catalogue integration.
  B2-03 checks selected revision/checksum/export/closure; B2-06/B2-07 connect
  retained verification. Source wiring is not activation or acceptance.
- New validator tasks never fall back to mutable Recipe rows because qualified
  selection is missing. Valid already selected old profiles remain readable
  under their exact original contracts; that is retention, not fallback.
- `subject_json` and actual typed observed rule/results follow B1-03. Historical
  three-field content bodies do not override the complete-subject input boundary.
- A positive stored flag alone is not trusted evidence. The operator's posted
  human/seed/class label cannot choose provenance or bypass Q1/behavior/Q2.
- A Recipe/pure-code subject does not own a Skill association. B1-06 adds the
  actual typed class/profile dispatch and distinct store writers; it does not
  assume that a generic handler already exists or invent per-class API families.
- Immutable revisions/evidence are not repaired through legacy mutable upserts.
  B2-09 owns checked boot/upgrade/repair and preservation of overrides/old pins.
- Generic classes exclude protected class 10 and historical class 50. No new
  Scaffold validator is required. Dynamic occurrences remain V35-13/14 work;
  approval/profile changes cannot relax existing flat occurrence fencing.
- No new validator scheduler/outbox/queue or Rust workflow loop is created.
  B3-06/B3-07 extend the existing shared durable work owner, consuming B1 subjects
  and B2 selections. One global Monty remains the execution service.

## 4. Historical source and prior review records

The complete pre-consolidation **working-tree** text, including the other
agent's uncommitted revisions and both correction records, is preserved
byte-for-byte as [historical text](docs/archive/missing4_v3_at_k3.pre-bob-2026-11-03.txt).
That archive is historical evidence/navigation only and must not be implemented
as current instructions. Earlier committed versions remain in Git history.

Archive identity before consolidation:

- SHA-256: `083b8cf4e485726b04c231ab42e91cce040e82e65be7b572a82d2630ec5bf2ae`.
- Size: 41,103 bytes.
- Consolidation baseline: `958cbb9c86d50644c59fcd40ced9c5a7cafc9cdc` plus the
  archived uncommitted working text, not a claim of source/build acceptance.

No implementation checkbox was closed, no source was changed, and no native
validation was performed by superseding this document. Continue in the named
Bob steps and record real outcomes there under the existing development policy.
