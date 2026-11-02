# TASKS — Active and Completed Task Registry

| ID | Title | Package | Status | Assigned | Completed | Blocked by |
|----|-------|---------|--------|----------|-----------|------------|
| TASK-002 | Investigate PgCompositionPort production wiring scope | PKG-G | DONE | 2026-11-01 | 2026-11-01 | none |
| TASK-003 | Remove run_q1_validation legacy path | PKG-H | DONE | 2026-11-01 | 2026-11-01 | none |
| TASK-004 | Integration test T1–T5 for run_q1_validation_retained | PKG-I | DONE | 2026-11-01 | 2026-11-01 | none |
| TASK-005 | Qualify L013–L025 shell/git component exports | PKG-L-A | DONE | 2026-11-01 | 2026-11-02 | none |
| TASK-006 | PgCompositionPort production wiring — locate constructor chain | PKG-G2 | CLOSED | 2026-11-01 | 2026-11-02 | Invalid premise: compose is read-only; no validator_bridge needed |
| TASK-007 | PgCompositionPort wire into global_task_factory drive path | PKG-G3 | CLOSED | 2026-11-01 | 2026-11-02 | Invalid premise: Q1 kicks already fire from create_and_submit paths |
| TASK-008 | Add validator_bridge to PgCompositionPort struct and constructor | PKG-G3 | CLOSED | 2026-11-02 | 2026-11-02 | Invalid: compose path is read-only; no Q1 kick needed |
| TASK-009 | Audit and qualify L026–L050 component body exports | PKG-L-B | DONE | 2026-11-02 | 2026-11-02 | none |
| TASK-010 | Wave-3 audit: identify next bounded packages | PKG-AUDIT-3 | DONE | 2026-11-02 | 2026-11-02 | none |
| TASK-011 | Commit implementation-status update for wave-2 audit findings | PKG-DOC | DONE | 2026-11-02 | 2026-11-02 | none |
| TASK-012 | L010 builtin.file_exists: Rust primitive + schema + seed rows + test updates | PKG-L010 | DONE | 2026-11-02 | 2026-11-02 | none |
| TASK-013 | content_checksum for pc_row()/skill_row() helpers + ToolSkill auto-compute | PKG-TS-CHECKSUM | DONE | 2026-11-02 | 2026-11-02 | none |
| TASK-014 | L010 file_exists in bootstrap_l013_approval TOOL_CALLING_PAIRS | PKG-L010-APPROVAL | DONE | 2026-11-02 | 2026-11-02 | none |
| TASK-015 | Validator Step 7-A: class-specific PC bodies for all 8 class routes | PKG-VALIDATOR-STEP7 | DONE | 2026-11-02 | 2026-11-02 | none |
| TASK-016 | L001–L006 filesystem + L011–L012 path helpers in bootstrap_l013_approval | PKG-L001-APPROVAL | DONE | 2026-11-02 | 2026-11-02 | none |
| TASK-017 | Trigger management PairSpecs in bootstrap_l013_approval | PKG-TRIGGER-APPROVAL | DONE | 2026-11-01 | 2026-11-01 | none |
| TASK-018 | Validator Step 7-B: negative fixture tests per class route | PKG-VALIDATOR-STEP7B | IN_PROGRESS | 2026-11-01 | — | TASK-015 ✓ |
| TASK-019 | Security grep leaf skill pairing + 3 PairSpecs | PKG-SECURITY-APPROVAL | IN_PROGRESS | 2026-11-01 | — | TASK-017 ✓ |
| TASK-020 | ITEM-C: build monty_worker + run T2–T5 integration tests | PKG-MONTY-TESTS | IN_PROGRESS | 2026-11-01 | — | none |

<!-- Append rows as tasks are created/completed -->
