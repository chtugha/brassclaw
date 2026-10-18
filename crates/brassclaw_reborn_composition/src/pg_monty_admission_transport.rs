// Actual shared claim/journal and service lifecycle assembly. The production
// admission owner also installs its task-bound native model response journal.
include!("pg_monty_admission_core.rs");
include!("pg_monty_admission_lifecycle.rs");
