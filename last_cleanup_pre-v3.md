# Last Cleanup Before v3 — Pre-v3 Release Checklist

## Purpose

This plan collects all outstanding work from 16 subplan/plan_stub files:
- Mark plans that are already implemented (status-only updates)
- Implement the one genuinely unimplemented item: dead factory production path
- Fix the `TrustDataMissing` dead variant
- Verify all stubs are replaced
- Run full clippy + tests, tag v3-ready

## Assessment (as of this session)

### Already fully implemented in codebase — just need plan-file status updates:

| Plan file | Evidence |
|-----------|---------|
| `subplan_step3.2_of_checkup.md` | `auto_validate_pending` in `pg_recipe_store.rs:1530`, `spawn_q1_validation_sweep` in `retention_sweep.rs:168` |
| `subplan_step4_1_trust_layer_removal.md` | `SkillTrustLevel` completely absent from codebase; `TrustDataMissing` still present as unreachable variant in `SkillActivationSelectionError` (separate enum, kept for exhaustive matching) |
| `subplan_step5.3_of_checkup.md` | `PgRecipeStoreFacade` implements all 13 `RecipeStore` methods; wired in `webui.rs:218`; `PgRecipeLibrary` wired in `runtime.rs:2388` |
| `subplan_step6_1_retrieval_source.md` | `retrieval_source.rs` present with `RetrievalSource`, `ComponentItem`, `PostgresSource`, `RamSource` |
| `subplan_step65_component_import.md` | `component_import.rs` (16KB) present and wired |
| `subplan_step67_fetch_for_turn.md` | `fetch_for_turn` + `FetchForTurnResult` present in `retrieval_source.rs` |
| `subplan_step6_3_monty_vm_settings.md` | `PgMontyVmSettingsStore` present, wired in `webui.rs`, all 4 methods functional |
| `subplan_step81_intent_inputs_api.md` | `PgIntentInputsStore` present, 3 REST routes in `handlers.rs` |
| `subplan_step9_1_bundled_skills_removal.md` | `bundled_skills` gated `#[cfg(not(feature = "skills-db"))]` in `lib.rs:34`; `build.rs:38` gates embedding |
| `plan_stub_step63_max_duration_wiring.md` | `max_turn_duration` in `DefaultPlannedRuntimeConfig` (runtime.rs:109); enforced in `turn_runner.rs:402` |
| `subplan_pg4_factory_wiring.md` | All sub-steps resolved |
| `subplan_pg4_runtime_pg_path.md` | All steps implemented |
| `subplan_pg4_steps4to9_runtime_pg_path.md` | Marked IMPLEMENTED |
| `subplan_pg4_wiring_of_checkup.md` | Steps 1–7 complete |

### Genuinely remaining (need code work):

| Plan file | What's left |
|-----------|-------------|
| `plan_stub_factory_production_path.md` | Dead code chain deferred (Option C) — ACCEPTABLE as documented |
| `subplan_step4_1_trust_layer_removal.md` | `TrustDataMissing` variant in `SkillActivationSelectionError` — unreachable but present. Decide: remove or keep with clear comment |
| `subplan_step6_10_doctype_retire.md` | Partial (DocType deprecated, context.rs update deferred to PG-8) — ACCEPTABLE |

---

## Steps

### Step 1 — Mark all already-implemented subplan files as done ✅ DONE

Update status headers of the 10 plan files listed in "Already fully implemented":
- `subplan_step3.2_of_checkup.md` → `## Status: ✅ IMPLEMENTED`
- `subplan_step4_1_trust_layer_removal.md` → `## Status: ✅ IMPLEMENTED (SkillTrustLevel removed; TrustDataMissing kept as exhaustive-match placeholder)`
- `subplan_step5.3_of_checkup.md` → `## Status: ✅ IMPLEMENTED`
- `subplan_step6_1_retrieval_source.md` → `## Status: ✅ IMPLEMENTED`
- `subplan_step65_component_import.md` → `## Status: ✅ IMPLEMENTED`
- `subplan_step67_fetch_for_turn.md` → `## Status: ✅ IMPLEMENTED`
- `subplan_step6_3_monty_vm_settings.md` → `## Status: ✅ IMPLEMENTED`
- `subplan_step81_intent_inputs_api.md` → `## Status: ✅ IMPLEMENTED`
- `subplan_step9_1_bundled_skills_removal.md` → `## Status: ✅ IMPLEMENTED`
- `plan_stub_step63_max_duration_wiring.md` → already says ✅ IMPLEMENTED

### Step 2 — Remove `TrustDataMissing` dead variant from `SkillActivationSelectionError` ✅ DONE

File: `crates/brassclaw_first_party_extension_ports/src/activation.rs`

`TrustDataMissing` is an unreachable variant in `SkillActivationSelectionError`.
The comment says it was "kept for exhaustive-match compatibility" — but if there are
no callers that can produce this error, it should be removed cleanly.

**Action**: 
1. Check whether any code still produces `TrustDataMissing` (creates `Self::TrustDataMissing`)
2. If no producer: remove the variant and its match arm
3. Update `skill_activation.rs` match arm that handles it

### Step 3 — Verify plan_stub_factory_production_path.md decision documented ✅ DONE

The dead `build_production_shaped` chain is accepted as Option C (deferred).
Confirm the plan file documents this clearly and has no ⚠️ blocking items.
No code changes needed.

### Step 4 — Verify step 6.10 deferred state is correctly documented ✅ DONE

`subplan_step6_10_doctype_retire.md` says DocType deletion is deferred to PG-8.
`DocType` is `#[deprecated]`. Confirm this matches checkup.md.

### Step 5 — Run full clippy + tests ✅

```bash
cargo clippy --all --benches --tests --examples --all-features -- -D warnings
cargo test
```

### Step 6 — Commit and push all changes ✅
