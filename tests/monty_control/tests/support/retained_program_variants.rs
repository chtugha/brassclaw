//! Variant constructors shared only by the retained-execution and Q1 callers.
use super::retained_program;
use brassclaw_engine::memory::retained_tools::RetainedToolProgram;
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use serde_json::Value;
use std::sync::Arc;

pub(super) async fn program_with_source(
    store: &PgComponentRevisionStore,
    invalid_output: bool,
    source: Option<&str>,
) -> Arc<RetainedToolProgram> {
    retained_program::build_program(store, invalid_output, source, false, None).await
}
pub(super) async fn program_with_preload_source(
    store: &PgComponentRevisionStore,
    source: &str,
    exports: Option<Value>,
) -> Arc<RetainedToolProgram> {
    retained_program::build_program(store, false, Some(source), true, exports).await
}
