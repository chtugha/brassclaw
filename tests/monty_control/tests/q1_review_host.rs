//! Standalone runner for the shared actual-worker review evidence.

#[path = "support/admission.rs"]
mod native_admission_fixture;

#[path = "support/q1_review_fixture.rs"]
mod fixture;
#[path = "../../../crates/brassclaw_reborn_composition/src/pg_monty_admission_core.rs"]
mod pg_monty_admission;

// Same observation parser as Composition; never a native qualification source.
#[path = "../../../crates/brassclaw_reborn_composition/src/task_model_capture_record.rs"]
mod task_model_capture_record;

#[path = "../../../crates/brassclaw_reborn_composition/src/monty_result_reference.rs"]
mod monty_result_reference;
