//! Engine-side port over the component store + composition system (the IBS).
//!
//! `ComponentPort` consolidates the host-call surface that reads & composes
//! saved-plan components: `host.resolve_intent`, `host.fetch_component`,
//! `host.resolve_component_by_name`, `host.list_skills`, and
//! `host.compose_orchestrator`. The engine handlers thin-call this port; the
//! composition layer owns the impl (`PgCompositionPort`) because it is the sole
//! crate that sees both `brassclaw_engine` (for `build_instruction` +
//! `compose_program` + the component/intent free fns) and the Postgres pool.
//! This mirrors the [`crate::executor::DynamicToolPort`] and
//! [`crate::memory::RetrievalSource`] engine↔composition port precedent.
//!
//! `brassclaw_engine` cannot depend on `brassclaw_host_runtime` (the host
//! runtime is a system-service crate downstream of the engine), and the cdylib
//! *loader* (`DynamicToolLoader`) that ultimately applies a composed program's
//! `rust_directives` lives there. The composition layer owns the impl for the
//! same reason.
//!
//! # Contract
//!
//! `host.compose_orchestrator(component_id, step_link, user_input)` thin-calls
//! [`ComponentPort::compose`], which:
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
//! Until the composition impl is wired, the engine passes `None` and every
//! handler degrades gracefully (no_match / Null / empty list /
//! `{ok:false, error:"composition_unavailable"}`).
//!
//! # Feature gate
//!
//! The trait + error + return types are UNGATED (`IntentResolution`,
//! `ComponentItem`, `ComposedProgram`, `ComponentScope`, `Thread` are all
//! always-available engine types). Only the `PgCompositionPort` IMPL is
//! `skills-db`-gated (the engine free fns it delegates to are gated). The
//! handlers therefore shed their `#[cfg]` wrappers and compile under both
//! configs, degrading to null/no_match when the port is `None`.

use thiserror::Error;

use crate::memory::composition::ComposedProgram;
use crate::memory::intent_system::IntentResolution;
use crate::memory::retrieval_source::{ComponentItem, ComponentScope};
use crate::types::thread::Thread;

/// Errors raised by a [`ComponentPort`] implementation.
#[derive(Debug, Clone, Error)]
pub enum ComponentPortError {
    /// No component bridge is wired (`None` port) — the orchestrator falls
    /// back to Non-Matching-Mode / the LLM path / an empty skill list.
    #[error("component bridge unavailable")]
    Unavailable,
    /// The recipe (class 21) row for `component_id` was not found in scope.
    #[error("recipe {component_id} not found")]
    RecipeNotFound { component_id: String },
    /// No variant matched the supplied `step_link`.
    #[error("no variant matched step_link {step_link}")]
    NoVariantMatch { step_link: String },
    /// A DB / IBS-compile / intent-resolution failure.
    #[error("component failure: {reason}")]
    Failure { reason: String },
}

/// Engine-side port over the component store + composition system (the IBS).
/// The implementation lives in the composition layer (`PgCompositionPort`) and
/// delegates to the engine component/intent free fns + `build_instruction` +
/// `compose_program` (+, eventually, `DynamicToolLoader` for `rust_directives`).
///
/// `async` because every method drives the DB pool — must not be `block_on()`-ed
/// inside a running Tokio runtime (mirrors [`crate::memory::RetrievalSource`]).
pub trait ComponentPort: Send + Sync {
    /// Resolve the user's input to a component match / disambiguation / no-match
    /// (`host.resolve_intent`). The handler serializes the [`IntentResolution`]
    /// into a Monty dict.
    fn resolve_intent(
        &self,
        scope: &ComponentScope,
        user_input: &str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<IntentResolution, ComponentPortError>>
                + Send
                + '_,
        >,
    >;

    /// Fetch a single validated component by UUID + class code
    /// (`host.fetch_component`). `Ok(Some)` → the matched [`ComponentItem`];
    /// `Ok(None)` → absent / SEC-01-unvalidated.
    fn fetch_component(
        &self,
        scope: &ComponentScope,
        component_id: uuid::Uuid,
        class_code: i32,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Option<ComponentItem>, ComponentPortError>>
                + Send
                + '_,
        >,
    >;

    /// Fetch a single validated component by name + class code — the §0.9
    /// Option B fallback (`host.resolve_component_by_name`). Same return shape
    /// as [`ComponentPort::fetch_component`].
    fn resolve_component_by_name(
        &self,
        scope: &ComponentScope,
        name: &str,
        class_code: i32,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Option<ComponentItem>, ComponentPortError>>
                + Send
                + '_,
        >,
    >;

    /// List all skills visible to the thread (`host.skill_list` /
    /// `__list_skills__`). Skills-db fast path (sorted `reborn_skills`) with a
    /// MemoryDoc `Store` fallback; the handler serializes the `Vec<Value>` into
    /// a Monty list.
    fn list_skills(
        &self,
        thread: &Thread,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<serde_json::Value>, ComponentPortError>>
                + Send
                + '_,
        >,
    >;

    /// Compose the recipe (`component_id`) + variant (`step_link`) into the
    /// predefined [`ComposedProgram`], binding `{{vars.NAME}}` slots captured
    /// from `user_input` (`host.compose_orchestrator`). The handler serializes
    /// the result into a Monty dict.
    fn compose(
        &self,
        scope: &ComponentScope,
        component_id: uuid::Uuid,
        step_link: &str,
        user_input: &str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ComposedProgram, ComponentPortError>>
                + Send
                + '_,
        >,
    >;
}
