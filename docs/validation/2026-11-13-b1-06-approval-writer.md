# B1-06 Approval-Writer — package handoff report (rev 5)

Date: 2026-11-13.
Branch: `codex/b1-06-approval-writer`.
Worktree: `/private/tmp/brassclaw-b1-06`.
Base commit: `d24a87508` (Resolve final Cargo manifest warnings…).

**Status: repair checkpoint — authored path closed.**
The writer infrastructure and all system-seed paths are complete. The authored
pure-component approval path is explicitly closed until the trusted behavioral
and human-Q2 evidence writers are implemented (remaining B1-06 work). This
handoff covers the closed-path state; B1-06 is not fully done.

## Commits (atop `d24a87508`)

| SHA | Description |
|-----|-------------|
| `19e80d013` | Repair approval writer: authored evidence format + Q1 run check |
| `7e9706965` | Handoff report rev 1 |
| `e62a4db6c` | Full subject binding for authored evidence |
| `266bea0d7` | Scope Q1 run binding to q1-kind; legacy format for behavior/Q2 |
| `ac345c655` | Handoff report rev 2 |
| `d70f23958` | Close authored path until qualified producers connected |
| `825591821` | Revise tests to reflect closed authored path |
| `c0cdb7ccd` | Handoff report rev 3 |
| `b48d45b7c` | Apply review corrections; mark as repair checkpoint |
| `3f802f79c` | *(regression — do not merge; superseded by 01cc4499a)* |
| `01cc4499a` | Restore b48d state; apply actual B1 findings |
| *(this file)* | Handoff report rev 5 |

**Note:** `3f802f79c` stripped the authored-path guard, `EvidenceFormat`,
`detect_evidence_format`, `validate_legacy_evidence_components` and the Q1
subject-binding infrastructure without connecting qualified replacement
producers. It must not be merged. `01cc4499a` restores the correct state from
`b48d45b7c` and applies the actual B1 findings on top.

---

## Scope

This package owns the repairs described in §B1-06: restricting successful
approval creation to the existing trusted writer, verifying that authored
approvals are bound to the exact component revision and review submission that
produced the evidence, accepting both legacy bootstrap and authored validator
evidence formats, and explicitly closing the authored path until its trusted
behavioral and human-Q2 evidence writers are implemented.

No changes are made to `pg_pure_approval_store.rs`, `handlers.rs`, `router.rs`,
`reborn_services.rs` or any other file outside the three changed files. The
public `resolve_existing_pure_component_approval` path is unchanged.

---

## Changed files

| File | Kind |
|------|------|
| `crates/brassclaw_pg/migrations/V214__pure_approval_q1_run_binding.sql` | New migration |
| `crates/brassclaw_reborn_composition/src/pg_pure_component_approval.rs` | Repaired writer |
| `crates/brassclaw_reborn_composition/src/pg_pure_component_approval_tests.rs` | Revised tests |

---

## V214 migration — `pure_approval_q1_run_binding`

Adds a `BEFORE INSERT` trigger `pure_approval_q1_run_check` on
`reborn_pure_component_approvals`. For every new **authored**
(`validation_mode = 'authored'`) row, the trigger checks that `q1_ref` exists
in `reborn_validator_q1_runs` with `succeeded = TRUE`. `system_seed` rows are
exempt. Pre-existing rows are not retroactively validated (B2-09 owns the
historical audit). No change to any existing table or applied migration.

---

## Writer — `pg_pure_component_approval.rs`

### Two evidence formats

`detect_evidence_format` distinguishes the two `/1` evidence formats by the
presence of mutually exclusive context fields. Both match arms use a single
combined condition (`||`) — no nested `if`:

| Format | Fields | Discriminator |
|--------|--------|---------------|
| Legacy (9-field) | `association_checksum`, `components`, … | `association_checksum` present |
| Authored (12-field) | `submission_id`, `subject_checksum`, `validator_selection_checksum`, `class_code`, `task_handle`, … | all five authored fields present |

A document with both or neither is rejected.

### Format routing by stage kind

| Stage kind | Accepted format | Reason |
|------------|-----------------|--------|
| `q1` | Both legacy and authored | `persist_validator_q1_evidence` writes authored; bootstrap writes legacy |
| `behavior` | **Legacy only** | Trusted pure-behavior writer not yet implemented (B1-06 remaining) |
| `human_q2` | **Legacy only** | Trusted pure/workflow-Q2 writer not yet implemented (B1-06 remaining) |

### Authored path status — **closed** (repair checkpoint)

`record_pure_component_approval` immediately rejects all `authored` mode
declarations:

```
authored pure-component approvals are not yet accepted:
qualified behavioral and human-Q2 producers are not connected
```

The Q1 subject-binding infrastructure (join of `reborn_validator_q1_runs` with
`reborn_component_review_submissions`, all five authored field checks) is
implemented in full below the guard and is activated by removing the early
return when the trusted writers are ready.

**Ownership:** implementing the trusted behavioral and Q2 evidence writers is
**remaining B1-06 work**. B3-09 owns their presentation and client binding.

### Source-only vs full Q1 — open design note for activation

`reborn_validator_q1_runs` rows are written by both
`persist_validator_q1_evidence` (full validator run) and
`persist_validator_source_evidence` (source-only prerequisite); both set
`succeeded = true`. The `reborn_component_validator_selections` table (V150,
extended by V210) carries either `scope = "preparation-only"` (format `/1`) or
`scope = "qualified-pure-source-prerequisite"` (format `/2`); **neither
represents full Q1**. The activation commit must require a **positive
qualification record from the Validator owner**, not a negative scope check.

---

## Tests — `pg_pure_component_approval_tests.rs`

All tests are in `#[cfg(all(test, feature = "skills-db"))]`. All test selectors
require `--features skills-db`.

| Test | Attribute | PG | Coverage |
|------|-----------|----|---------|
| `authored_approval_path_is_closed_until_producers_qualified` | `#[tokio::test]` | yes | authored guard fires before DB access |
| `missing_evidence_record_is_rejected` | `#[tokio::test]` | yes | system_seed reaches evidence-row lookup |
| `positive_flags_cannot_replace_exact_evidence_stage_or_revision` | `#[test]` | no | legacy evidence shape mutations; authored format accepted/rejected by kind |
| `authored_evidence_format_accepted_and_rejected_correctly` | `#[test]` | no | authored 12-field shape; nil UUIDs; wrong-length checksums; mixed format |
| `authored_q1_evidence_field_extraction_is_correct` | `#[test]` | no | five authored field mutations each cause rejection |
| `authored_q1_subject_binding_design_verified` | `#[tokio::test]` | yes | stages submission once (no duplicate revision); authored shape passes; guard fires |
| `positive_evidence_cannot_approve_an_omitted_immutable_dependency` | `#[tokio::test]` | yes | system_seed dependency graph check |
| `pure_link_rejects_a_different_approved_root_in_writer_and_database` | `#[tokio::test]` | yes | link writer + DB-level checks |
| `v214_trigger_rejects_authored_approval_without_succeeded_run` | `#[tokio::test]` | yes | V214 trigger: authored row without run → CHECK_VIOLATION; system_seed → accepted |
| `duplicate_stage_uuid_is_rejected_at_declaration_parse` | `#[test]` | no | q1_ref == behavioral_refs[0] → parser error |
| `evidence_format_rejects_unknown_fields_and_non_objects` | `#[test]` | no | extra unknown field + non-object base → rejected |

```bash
# All tests (PG-free tests complete without a database):
cargo test -p brassclaw_reborn_composition --features skills-db

# Individual filters:
cargo test -p brassclaw_reborn_composition --features skills-db \
  v214_trigger_rejects_authored_approval_without_succeeded_run

cargo test -p brassclaw_reborn_composition --features skills-db \
  authored_approval_path_is_closed_until_producers_qualified

cargo test -p brassclaw_reborn_composition --features skills-db \
  authored_q1_subject_binding_design_verified
```

---

## B1 findings resolved in `01cc4499a`

| Finding | Fix |
|---------|-----|
| Nested `if` in `detect_evidence_format` | Both match arms collapsed to single `||` condition |
| Duplicate revision staging in `authored_q1_subject_binding_design_verified` | Explicit `revisions.stage()` call removed; `root_rev = &sub.candidate` derived from submission result |
| V214 trigger not DB-tested | `v214_trigger_rejects_authored_approval_without_succeeded_run` added |
| Unknown-field rejection not tested | `evidence_format_rejects_unknown_fields_and_non_objects` added |
| Duplicate stage UUID not tested | `duplicate_stage_uuid_is_rejected_at_declaration_parse` added |

---

## Reused owners / no new subsystems

| Subject | Reused owner |
|---------|--------------|
| Revision snapshot check | `PgComponentRevisionStore::read_exact_in_transaction` (unchanged) |
| Evidence row reads | `reborn_component_review_evidence` (unchanged) |
| Validator run rows | `reborn_validator_q1_runs` V202 (read-only) |
| Submission rows | `reborn_component_review_submissions` V104 (read-only) |
| Validator selection rows | `reborn_component_validator_selections` V150/V210 (not read by this writer) |
| Transaction client | `bootstrap_evidence::io::EvidenceClient` (unchanged) |
| Approval/closure insert | unchanged SQL, same idempotency pattern |
| Public resolve path | `resolve_existing_pure_component_approval` unchanged |

---

## Validation results

### Format check

```
cargo fmt --check -p brassclaw_reborn_composition
# Exit 0 — confirmed on 01cc4499a
```

### Clippy

Not run in this session (skipped per orchestrator instruction). Required before
merge:

```bash
cargo clippy -p brassclaw_reborn_composition --all-targets \
  --features skills-db -- -D warnings
```

### Integration tests

Not executed — no PG in this session. Run with `--features skills-db` against a
real PG instance. See test table above.

---

## Known gaps and open points

1. **Trusted pure-behavior evidence writer (B1-06 remaining):** must produce
   behavioral evidence bound to a `reborn_component_review_submissions` subject
   for class-22 PythonCode before the authored path re-opens.
   `persist_behavioral_reviews` does not qualify. B3-09 owns presentation/client.

2. **Trusted pure/workflow-Q2 evidence writer (B1-06 remaining):** B3-09 owns
   presentation and client binding.

3. **Full Q1 qualification gate design (activation blocker, open):** `succeeded
   = true` on a run row is not sufficient (set by both full and source-only
   runs). `reborn_component_validator_selections` scope values do not represent
   full Q1. The activation commit must require a positive qualification record
   from the Validator owner.

4. **B2-03 exact-carriage reader:** reads approval by UUID only; does not verify
   subject binding. Separate B2-03 work.

5. **Historical authored rows pre-dating V214:** not retroactively validated.
   B2-09 owns the audit.

6. **B3-09 Q2 route authentication:** shape/reviewed_evidence verification done;
   operator-route authentication is B3-09 scope.

7. **Class-21 workflow-Q2 path:** not addressed here; class-22 only.

8. **Clippy under `--features skills-db` not run:** supervisor must run before
   merging.

---

## Contracts unchanged

- `resolve_existing_pure_component_approval` — unchanged.
- `record_pure_evidence_submission_link` — unchanged.
- `read_pure_component_approval_root` — unchanged.
- V200/V201/V202/V203 migrations — unchanged.
- `PureApprovalError` variants — unchanged.
- `PureComponentApprovalDeclaration` parser — unchanged.
- All callers in `bootstrap_validator_approval.rs` and
  `bootstrap_l013_approval.rs` — unchanged; `system_seed` declarations bypass
  both the authored guard and the new trigger.

---

## Integration prerequisites for supervisor

1. Run `cargo clippy -p brassclaw_reborn_composition --all-targets --features skills-db -- -D warnings`; confirm zero new warnings.
2. Run all tests with `--features skills-db` against a real PG instance; confirm all pass.
3. Verify no existing bootstrap integration tests regress.
4. Confirm `3f802f79c` is **not** merged; only `01cc4499a` and earlier commits on this branch are accepted.
5. Remaining B1-06 work (trusted behavioral and Q2 writers) must be tracked separately; this checkpoint does not close B1-06.
6. Orchestrator merges Cargo.lock, build scripts and documentation changes separately after review.
