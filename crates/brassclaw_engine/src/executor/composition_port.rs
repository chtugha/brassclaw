//! Step C.4.5.17 — engine-side port trait bridging the orchestrator's
//! `host.compose_orchestrator` host-call to the composition system (the IBS).
//!
//! `brassclaw_engine` cannot depend on `brassclaw_host_runtime` (the host
//! runtime is a system-service crate downstream of the engine), and the cdylib
//! *loader* (`DynamicToolLoader`) that ultimately applies a composed program's
//! `rust_directives` lives there. The composition layer is the sole crate that
//! sees both `brassclaw_engine` (for `build_instruction` + `compose_program`) and
//! `brassclaw_host_runtime` (for the loader), so it owns the impl. This mirrors
//! the [`crate::executor::DynamicToolPort`] and [`crate::memory::RetrievalSource`]
//! engine↔composition port precedent.
//!
//! # Contract
//!
//! `host.compose_orchestrator(component_id, step_link, user_input)` is a
//! `host.*` MethodCall handled in [`crate::executor::orchestrator`]. The handler
//! thin-calls [`CompositionPort::compose`], which:
//! 1. SELECTs the recipe (class 21) row by `component_id` + scope.
//! 2. Matches the variant by `step_link` (surfaced to Monty by
//!    `host.resolve_intent`, which already returns `step_link`).
//! 3. Runs the IBS `build_instruction(step_link, …)` → `BuildInstruction`.
//! 4. Resolves every included component UUID via a `ComponentResolver`
//!    (PythonCode→`executable_code`, Skill/ToolSkill→`skills`, Tool→`rust_directives`).
//! 5. Binds `{{vars.NAME}}` slot variables captured from `user_input`.
//! 6. Returns the predefined [`ComposedProgram`].
//!
//! The cdylib *application* of `rust_directives` (dlopen via `DynamicToolLoader`)
//! is a Step C.5/C.6 concern and is deferred — the directives are CARRIED in the
//! returned program so the driver/loader can apply them once that wiring lands.
//! Until the composition impl is wired, the engine passes `None` and the handler
//! degrades gracefully (`{ok:false, error:"composition_unavailable"}`).

use thiserror::Error;

use crate::memory::composition::ComposedProgram;
use crate::memory::retrieval_source::ComponentScope;

/// Errors raised by a [`CompositionPort`] implementation.
#[derive(Debug, Clone, Error)]
pub enum CompositionPortError {
    /// No composition bridge is wired (`None` port) — the orchestrator falls
    /// back to Non-Matching-Mode / the LLM path.
    #[error("composition bridge unavailable")]
    Unavailable,
    /// The recipe (class 21) row for `component_id` was not found in scope.
    #[error("recipe {component_id} not found")]
    RecipeNotFound { component_id: String },
    /// No variant matched the supplied `step_link`.
    #[error("no variant matched step_link {step_link}")]
    NoVariantMatch { step_link: String },
    /// A DB / IBS-compile failure during composition.
    #[error("composition failure: {reason}")]
    Failure { reason: String },
}

/// Engine-side port over the composition system (the IBS). The implementation
/// lives in the composition layer and delegates to `build_instruction` +
/// `compose_program` (+, eventually, `DynamicToolLoader` for `rust_directives`).
///
/// `async` because the backing recipe SELECT + IBS compile drive the DB pool —
/// must not be `block_on()`-ed inside a running Tokio runtime (mirrors
/// [`crate::memory::RetrievalSource`]).
pub trait CompositionPort: Send + Sync {
    /// Compose the recipe (`component_id`) + variant (`step_link`) into the
    /// predefined [`ComposedProgram`], binding `{{vars.NAME}}` slots captured
    /// from `user_input`. The handler serializes the result into a Monty dict.
    fn compose(
        &self,
        scope: &ComponentScope,
        component_id: uuid::Uuid,
        step_link: &str,
        user_input: &str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ComposedProgram, CompositionPortError>>
                + Send
                + '_,
        >,
    >;
}
