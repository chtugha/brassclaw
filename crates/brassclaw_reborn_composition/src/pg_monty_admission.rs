//! Global driver admission: shared claim/journal core plus actual service lifecycle.
include!("pg_monty_admission_transport.rs");

#[path = "pg_monty_model_response.rs"]
mod model_response;
