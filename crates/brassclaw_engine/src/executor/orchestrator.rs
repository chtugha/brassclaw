//! Python orchestrator — the self-modifiable execution loop.
//!
//! Replaces the Rust `ExecutionLoop::run()` with versioned Python code
//! executed via Monty. The orchestrator is the "glue layer" between the
//! LLM and tools — tool dispatch, output formatting, state management,
//! truncation — all in Python, patchable by the self-improvement validation loop.
//!
//! Host functions exposed to the orchestrator Python:
//! - `__check_signals__` — poll for stop/inject signals
//! - `__emit_event__` — broadcast a ThreadEvent
//! - `__save_checkpoint__` — persist thread state
//! - `__transition_to__` — change thread state (validated)
//! - `__retrieve_docs__` — query memory docs
//! - `__assemble_prior_knowledge__` — retired (v3 Phase H8.4); replaced by the
//!   `pub` `assemble_prior_knowledge_with_hint` library call (§3.13/§3.14)
//! - `__check_budget__` — remaining tokens/time/USD
//! - `__get_actions__` — available tool definitions
//!
//! C.1 first-class callables: tools are also bound as `host.<name>(...)` in the
//! Monty namespace (the `host` Dataclass → `MethodCall` path). The
//! `__execute_action__` / `__execute_code_step__` / `__execute_actions_parallel__`
//! meta-primitives are RETIRED — a recipe step calls the ToolSkill directly via
//! `host.X(...)`, and "call N tools" is a sequential recipe with N steps
//! (Monty is single-threaded, so a parallel helper would degrade to sequential
//! anyway). C.2 reclassifies the remaining `__*__` arms; C.7 retires this
//! `execute_orchestrator` fn itself once C.6 activates the cross-turn driver.

use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use std::collections::HashMap;

use monty::{
    DictPairs, ExtFunctionResult, FunctionCall, LimitedTracker, MontyObject, MontyRun,
    NameLookupResult, PrintWriter, ResourceLimits, RunProgress,
};
use tracing::{debug, warn};

use super::dynamic_tool_port::DynamicToolPort;
use super::scripting::{execute_code, json_to_monty, monty_to_json, monty_to_string};
use super::thread_context::thread_execution_context;
use crate::capability::lease::LeaseManager;
use crate::capability::policy::PolicyEngine;
use crate::memory::{ComponentScope, RetrievalEngine, RetrievalSource};
use crate::runtime::lease_refresh::reconcile_dynamic_tool_lease;
use crate::runtime::messaging::{SignalReceiver, ThreadOutcome, ThreadSignal};
use crate::traits::effect::EffectExecutor;
use crate::traits::llm::LlmBackend;
use crate::traits::store::Store;
use crate::types::error::{EngineError, OrchestratorFailure, OrchestratorFailureKind};
use crate::types::event::{EventKind, ThreadEvent};
use crate::types::message::ThreadMessage;
use crate::types::project::ProjectId;
use crate::types::shared_owner_id;
use crate::types::step::{ActionCall, StepId, TokenUsage};
use crate::types::thread::{Thread, ThreadState};

/// Stable Python-level `type_id` for the injected `host` namespace Dataclass (C.1).
/// `host.<tool>(...)` compiles to `CallAttr`; the dataclass `py_call_attr` routes any
/// public attr not in attrs to `MethodCall`, surfacing as a `FunctionCall` with the
/// bare tool name and `method_call = true`. The id is arbitrary — it never keys into
/// Monty's type table; it only identifies the `host` object for repr/equality.
const HOST_NAMESPACE_TYPE_ID: u64 = 0x484F_5354; // "HOST"

/// The compiled-in default orchestrator (v0).
pub(crate) const DEFAULT_ORCHESTRATOR: &str = include_str!("../../orchestrator/basic_mode.py");

/// Well-known title for orchestrator code in the Store.
pub const ORCHESTRATOR_TITLE: &str = "orchestrator:main";

/// Well-known tag for orchestrator code docs.
pub const ORCHESTRATOR_TAG: &str = "orchestrator_code";

/// Outcome of a Tier-0 (no-LLM) recipe execution, surfaced from the
/// Tier-0 recipe branch through [`OrchestratorResult`] (v3 Phase H4.6,
/// Q-H7 Architecture A). `Some` only when the turn went through the
/// Tier-0 recipe branch: success is read from the `complete_result(
/// extra={"tier_zero_outcome": {"recipe_id","success":true}})` stamp
/// (Q-H6); failure is read by scanning `thread.events` for
/// [`EventKind::RecipeTierZeroFailed`]. The composition event listener
/// (H4.7) ALSO independently fires `record_recipe_outcome(recipe_id,
/// success)` off the terminal `RecipeTierZeroSucceeded` /
/// `RecipeTierZeroFailed` event — this field is for engine-internal
/// consumers + unit tests (the listener is the durable recording path).
///
/// Reused by Model B/C (v3 Phase H.5 O4): the stamp + events are
/// produced by the agent-loop Tier-0 path (the `LoopOrchestratorPort`
/// driver + the engine `pub` fns extracted in H.8). The Model A
/// `default.py` step-0 `tier_zero` branch that previously wrote this
/// stamp was removed in v3 Phase H.5 O3.
#[derive(Debug, Clone)]
pub struct TierZeroOutcome {
    /// The matched Recipe component UUID (class 21) as a string.
    pub recipe_id: String,
    /// `true` when the Tier-0 channel ran all steps to success;
    /// `false` when it failed (and the turn degraded to Tier-2 LLM).
    pub success: bool,
}

/// Result of running the orchestrator.
pub struct OrchestratorResult {
    /// The thread outcome parsed from the orchestrator's return value.
    pub outcome: ThreadOutcome,
    /// Total tokens used by LLM calls within the orchestrator.
    pub tokens_used: TokenUsage,
    /// Tier-0 recipe execution outcome, `Some` only when the turn went
    /// through the Tier-0 recipe branch (v3 Phase H4.6). Built by
    /// [`build_tier_zero_outcome`] in the `RunProgress::Complete` arm.
    pub tier_zero_outcome: Option<TierZeroOutcome>,
}

/// Outcome of driving a [`MontySession`] one step. `Complete` carries the
/// orchestrator result; `AwaitNextTurn` means the script called
/// `host.await_next_turn()` and the VM is parked, awaiting the next turn's
/// input to resume. The non-persistent [`execute_orchestrator`] caller maps
/// `AwaitNextTurn` to an error; the cross-turn-persistent driver (C.6 slice 4)
/// parks the session in a conversation-keyed registry and resumes it next turn.
pub enum OrchestratorYield {
    /// The orchestrator finished and produced a result.
    Complete(Box<OrchestratorResult>),
    /// The orchestrator called `host.await_next_turn()` and is parked.
    AwaitNextTurn,
}

/// Build the [`TierZeroOutcome`] for a completed orchestrator turn (v3
/// Phase H4.6). Pure + unit-testable without driving the whole VM:
/// - SUCCESS: read from the `complete_result(extra={"tier_zero_outcome":
///   {"recipe_id","success":true}})` stamp (Q-H6) — the stamp is ONLY
///   ever written on the Tier-0 success path, so its presence with a
///   `recipe_id` IS the success signal → `success == true`.
/// - FAILURE: scan `events` for [`EventKind::RecipeTierZeroFailed`]
///   carrying a non-empty `recipe_id` → `success == false` (the failure
///   signal rides the event, NOT a result-dict stamp, per Q-H6 — a
///   failure degrades to Tier-2 and the result dict has no stamp).
/// - otherwise `None` (a plain Tier-2 LLM turn, no Tier-0 attempt).
///
/// Reused by Model B/C (v3 Phase H.5 O4): the stamp + events this reads
/// are produced by the agent-loop Tier-0 path (the `LoopOrchestratorPort`
/// driver + the engine `pub` fns extracted in H.8); the Model A
/// `default.py` writer was removed in v3 Phase H.5 O3.
pub fn build_tier_zero_outcome(
    result: &serde_json::Value,
    events: &[ThreadEvent],
) -> Option<TierZeroOutcome> {
    // (1) success via the `extra` stamp (only present on the success path).
    if let Some(stamp) = result.get("tier_zero_outcome").and_then(|v| v.as_object())
        && let Some(recipe_id) = stamp.get("recipe_id").and_then(|v| v.as_str())
        && !recipe_id.is_empty()
    {
        return Some(TierZeroOutcome {
            recipe_id: recipe_id.to_string(),
            success: true,
        });
    }
    // (2) failure via the RecipeTierZeroFailed event.
    for event in events {
        if let EventKind::RecipeTierZeroFailed { recipe_id, .. } = &event.kind
            && !recipe_id.is_empty()
        {
            return Some(TierZeroOutcome {
                recipe_id: recipe_id.clone(),
                success: false,
            });
        }
    }
    // (3) plain Tier-2 turn — no Tier-0 attempt.
    None
}

fn normalize_pause_outcome(
    thread: &mut Thread,
    outcome: &ThreadOutcome,
) -> Result<(), EngineError> {
    if matches!(outcome, ThreadOutcome::GatePaused { .. }) && thread.state != ThreadState::Waiting {
        thread.transition_to(
            ThreadState::Waiting,
            Some("waiting on external gate resolution".into()),
        )?;
    }
    Ok(())
}

/// Default orchestrator VM wall-clock budget, in seconds.
const ORCHESTRATOR_DEFAULT_MAX_DURATION_SECS: u64 = 300;
/// Floor for the configurable orchestrator budget, to prevent nonsense values.
const ORCHESTRATOR_MIN_MAX_DURATION_SECS: u64 = 30;
/// Ceiling for the configurable orchestrator budget, bounding resource waste.
const ORCHESTRATOR_MAX_MAX_DURATION_SECS: u64 = 3600;

/// Resolve the orchestrator VM wall-clock budget.
///
/// **DB-less fallback only.** In a full DB-backed deployment the duration is
/// read from `reborn_monty_vm_settings.max_duration_secs` (Phase 6 wiring via
/// `MontyVmSettings`). `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` is retained
/// solely as a last-resort override for the `RamSource` / DB-less code path.
/// Do not rely on this env var in production deployments.
fn orchestrator_max_duration() -> std::time::Duration {
    static CACHED: OnceLock<std::time::Duration> = OnceLock::new();
    *CACHED.get_or_init(|| {
        let secs = std::env::var("BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(ORCHESTRATOR_DEFAULT_MAX_DURATION_SECS)
            .clamp(
                ORCHESTRATOR_MIN_MAX_DURATION_SECS,
                ORCHESTRATOR_MAX_MAX_DURATION_SECS,
            );
        std::time::Duration::from_secs(secs)
    })
}

/// Maximum allocation steps allowed per orchestrator VM execution.
const ORCHESTRATOR_MAX_ALLOCATIONS: usize = 5_000_000;

/// Classify a Monty orchestrator failure into a typed
/// [`OrchestratorFailure`] that carries a user-safe classification plus
/// the preserved low-level detail for gateway debug mode.
///
/// The raw `err_msg` (often a Python traceback containing internal file
/// paths and upstream HTTP bodies) is always stored on the returned
/// struct's `debug_detail` field and emitted at `debug!`, never placed
/// into the user-visible classification — see
/// `.claude/rules/error-handling.md`, "Error Boundaries at the Channel
/// Edge" (#2546).
fn classify_orchestrator_failure(prefix: &str, err_msg: &str) -> OrchestratorFailure {
    debug!(prefix, err_msg, "orchestrator VM failure");

    let lower = err_msg.to_ascii_lowercase();
    // Reserve `TimeLimit` for unmistakable Monty wall-clock markers — the
    // user-facing message tells operators to raise
    // `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS`, which is wrong advice for
    // upstream LLM / network timeouts. Bare `"timeout"` / `"timed out"`
    // used to catch those (e.g. `reqwest`'s `"Request timed out"`,
    // provider `"Connection timed out"`) and point users at the budget
    // knob instead of the real failure class. Those now fall through to
    // `Other` (generic internal failure). References: serrrfirat review
    // on PR #2753, commit 82d06410.
    //
    // The predicates we keep are either the explicit env-var name in the
    // VM's own error text, the phrase the Monty runtime uses for its
    // duration limit, or the sentinel emitted by the engine when the
    // orchestrator itself times out a step. Duplicating `ResourceLimits`
    // wording is OK — those strings live alongside this classifier in the
    // same crate.
    let hit_time_limit = lower.contains("duration limit")
        || lower.contains("max_duration")
        || lower.contains("maximum duration")
        || lower.contains("execution duration exceeded")
        || lower.contains("orchestrator timed out");
    let hit_memory_limit = lower.contains("memory limit") || lower.contains("allocation limit");
    let hit_resource_limit = lower.contains("resource limit")
        || lower.contains("out of fuel")
        || lower.contains("fuel exhausted");
    let has_python_traceback =
        lower.contains("traceback (most recent call last)") || lower.contains("traceback:");

    let kind = if hit_time_limit {
        OrchestratorFailureKind::TimeLimit {
            prefix: prefix.to_string(),
            limit_secs: orchestrator_max_duration().as_secs(),
        }
    } else if hit_memory_limit || hit_resource_limit {
        OrchestratorFailureKind::ResourceLimit {
            prefix: prefix.to_string(),
        }
    } else if has_python_traceback {
        OrchestratorFailureKind::Traceback {
            prefix: prefix.to_string(),
        }
    } else {
        OrchestratorFailureKind::Other {
            prefix: prefix.to_string(),
        }
    };

    OrchestratorFailure::new(kind, err_msg)
}

/// Wrap a Monty VM panic (parse / start / resume phase) as a typed
/// orchestrator failure. The panic itself has no textual payload — the
/// `panic_payload` we can stringify is always a `&str` or `String` from
/// `catch_unwind` — so `debug_detail` carries the phase tag for
/// correlation.
fn orchestrator_vm_panic(prefix: &str, phase: &'static str) -> OrchestratorFailure {
    debug!(prefix, phase, "orchestrator VM panic");
    OrchestratorFailure::new(
        OrchestratorFailureKind::VmPanic {
            prefix: prefix.to_string(),
            phase,
        },
        format!("Monty VM panicked during {phase}"),
    )
}

/// Maximum consecutive failures before auto-rollback.
const MAX_FAILURES_BEFORE_ROLLBACK: u64 = 3;

/// Well-known title for orchestrator failure tracking.
const FAILURE_TRACKER_TITLE: &str = "orchestrator:failures";
const LEASE_REFRESH_WARN_INTERVAL_SECS: u64 = 60;

fn warn_on_lease_refresh_failure(context: &'static str, error: &crate::types::error::EngineError) {
    static LAST_WARN_TS: AtomicU64 = AtomicU64::new(0);

    let now = chrono::Utc::now().timestamp().max(0) as u64;
    let last = LAST_WARN_TS.load(Ordering::Relaxed);
    if now.saturating_sub(last) >= LEASE_REFRESH_WARN_INTERVAL_SECS
        && LAST_WARN_TS
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        warn!(context, error = %error, "dynamic lease refresh failed");
    } else {
        debug!(context, error = %error, "dynamic lease refresh failed");
    }
}

/// Load orchestrator code: runtime version from Store, or compiled-in default.
///
/// When `allow_self_modify` is false, always uses the compiled-in default
/// regardless of any runtime versions in the Store. This is the safe default
/// for production — runtime orchestrator patching is opt-in.
///
/// Checks the failure tracker — if the latest version has >= 3 consecutive
/// failures, falls back to the previous version (or compiled-in default).
pub async fn load_orchestrator(
    store: Option<&Arc<dyn Store>>,
    project_id: ProjectId,
    allow_self_modify: bool,
) -> (String, u64) {
    if !allow_self_modify {
        debug!("orchestrator self-modification disabled, using compiled-in default (v0)");
        return (DEFAULT_ORCHESTRATOR.to_string(), 0);
    }

    let Some(store) = store else {
        debug!("using compiled-in default orchestrator (v0, no store)");
        return (DEFAULT_ORCHESTRATOR.to_string(), 0);
    };

    let docs = match store.list_shared_memory_docs(project_id).await {
        Ok(d) => d,
        Err(_) => {
            debug!("using compiled-in default orchestrator (v0, store error)");
            return (DEFAULT_ORCHESTRATOR.to_string(), 0);
        }
    };

    load_orchestrator_from_docs(&docs, allow_self_modify)
}

/// Load orchestrator from pre-fetched system memory docs.
///
/// When the caller already has the `list_memory_docs` result, use this to
/// avoid a duplicate Store query. Returns `(code, version)`.
///
/// Respects `allow_self_modify` — when false, always returns the compiled-in
/// default. The caller in `loop_engine.rs` passes this from engine config.
pub fn load_orchestrator_from_docs(
    docs: &[crate::types::memory::MemoryDoc],
    allow_self_modify: bool,
) -> (String, u64) {
    if !allow_self_modify {
        return (DEFAULT_ORCHESTRATOR.to_string(), 0);
    }

    // Find all orchestrator versions, sorted by version number descending
    let mut versions: Vec<_> = docs
        .iter()
        .filter(|d| d.title == ORCHESTRATOR_TITLE && d.tags.contains(&ORCHESTRATOR_TAG.to_string()))
        .collect();
    versions.sort_by(|a, b| {
        let va = a
            .metadata
            .get("version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let vb = b
            .metadata
            .get("version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        vb.cmp(&va) // descending
    });

    if versions.is_empty() {
        debug!("using compiled-in default orchestrator (v0)");
        return (DEFAULT_ORCHESTRATOR.to_string(), 0);
    }

    // Check failure count for the latest version
    let failures = load_failure_count(docs);

    for doc in &versions {
        let version = doc
            .metadata
            .get("version")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        // Skip versions with too many failures (only check the latest)
        if version
            == versions[0]
                .metadata
                .get("version")
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
            && failures >= MAX_FAILURES_BEFORE_ROLLBACK
        {
            debug!(
                version,
                failures, "orchestrator version has too many failures, skipping"
            );
            continue;
        }

        debug!(version, "loaded runtime orchestrator");
        return (doc.content.clone(), version);
    }

    // All versions failed — fall back to compiled-in default
    debug!("all orchestrator versions failed, using compiled-in default (v0)");
    (DEFAULT_ORCHESTRATOR.to_string(), 0)
}

/// Record a failure for the current orchestrator version.
pub async fn record_orchestrator_failure(
    store: &Arc<dyn Store>,
    project_id: ProjectId,
    version: u64,
) {
    use crate::types::memory::{DocType, MemoryDoc};

    let docs = match store.list_shared_memory_docs(project_id).await {
        Ok(docs) => docs,
        Err(e) => {
            debug!("failed to list memory docs for failure tracker: {e}");
            return;
        }
    };
    let existing = docs.iter().find(|d| d.title == FAILURE_TRACKER_TITLE);

    let mut tracker = if let Some(doc) = existing {
        doc.clone()
    } else {
        MemoryDoc::new(
            project_id,
            shared_owner_id(),
            DocType::Note,
            FAILURE_TRACKER_TITLE,
            "",
        )
        .with_tags(vec!["orchestrator_meta".to_string()])
    };

    // Store failure count as JSON in content: {"version": N, "count": M}
    let current: serde_json::Value =
        serde_json::from_str(&tracker.content).unwrap_or(serde_json::json!({}));
    let current_version = current.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    let current_count = current.get("count").and_then(|v| v.as_u64()).unwrap_or(0);

    let new_count = if current_version == version {
        current_count + 1
    } else {
        1 // new version, reset count
    };

    tracker.content = serde_json::json!({
        "version": version,
        "count": new_count,
    })
    .to_string();
    tracker.updated_at = chrono::Utc::now();

    // The failure tracker carries the `orchestrator:` title prefix and is
    // therefore gated by `is_protected_orchestrator_doc` in the store.
    // Enter the trusted-internal-writes scope so the system-initiated save
    // is admitted without being mistaken for an LLM-authored patch.
    if let Err(e) =
        crate::runtime::with_trusted_internal_writes(store.save_memory_doc(&tracker)).await
    {
        debug!("failed to save orchestrator failure tracker: {e}");
    }

    debug!(version, count = new_count, "recorded orchestrator failure");
}

/// Reset the failure counter (called after successful execution).
pub async fn reset_orchestrator_failures(store: &Arc<dyn Store>, project_id: ProjectId) {
    let docs = store
        .list_shared_memory_docs(project_id)
        .await
        .unwrap_or_default();
    let existing = docs.iter().find(|d| d.title == FAILURE_TRACKER_TITLE);

    if let Some(doc) = existing {
        let mut tracker = doc.clone();
        tracker.content = serde_json::json!({"version": 0, "count": 0}).to_string();
        tracker.updated_at = chrono::Utc::now();
        // Same rationale as `record_orchestrator_failure`: the tracker doc
        // has an `orchestrator:` title so the store gate triggers. Enter
        // the trusted-writes scope for this system-initiated reset.
        let _ = crate::runtime::with_trusted_internal_writes(store.save_memory_doc(&tracker)).await;
    }
}

/// Load failure count for the latest orchestrator version.
fn load_failure_count(docs: &[crate::types::memory::MemoryDoc]) -> u64 {
    docs.iter()
        .find(|d| d.title == FAILURE_TRACKER_TITLE)
        .and_then(|d| serde_json::from_str::<serde_json::Value>(&d.content).ok())
        .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
        .unwrap_or(0)
}

/// Execute the orchestrator Python code with host function dispatch.
///
/// This is the core function that replaces `ExecutionLoop::run()`'s inner loop.
/// The orchestrator Python calls host functions via Monty's suspension mechanism,
/// and this function handles each suspension by delegating to the appropriate
/// Rust implementation.
///
/// `max_duration_override` — when `Some`, overrides the DB-less-fallback
/// `orchestrator_max_duration()` with the value read from
/// `reborn_monty_vm_settings.max_duration_secs` by the caller.
/// Pass `None` in DB-less / test contexts to use the env-var / compiled-in default.
#[allow(clippy::too_many_arguments)]
pub struct MontySession {
    progress: Option<RunProgress<LimitedTracker>>,
    parked_call: Option<FunctionCall<LimitedTracker>>,
    total_tokens: TokenUsage,
    final_result: Option<serde_json::Value>,
    stdout: String,
}

impl MontySession {
    /// Parse + start the orchestrator script. Extracts the setup phase of
    /// [`execute_orchestrator`]: builds the bootstrap inputs, compiles the
    /// script, resolves the resource budget, and runs the module top-level
    /// up to the first host call (or `Complete`). The session is then ready
    /// to be driven by [`MontySession::drive_to_yield`].
    pub fn new(
        code: &str,
        thread: &Thread,
        persisted_state: &serde_json::Value,
        max_duration_override: Option<std::time::Duration>,
    ) -> Result<Self, EngineError> {
        let total_tokens = TokenUsage::default();

        // Build context variables for the orchestrator
        let (input_names, input_values) = build_orchestrator_inputs(thread, persisted_state);

        // Parse and compile
        let runner = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            MontyRun::new(code.to_string(), "orchestrator.py", input_names)
        })) {
            Ok(Ok(runner)) => runner,
            Ok(Err(e)) => {
                return Err(EngineError::Orchestrator(classify_orchestrator_failure(
                    "Orchestrator parse error",
                    &e.to_string(),
                )));
            }
            Err(_) => {
                return Err(EngineError::Orchestrator(orchestrator_vm_panic(
                    "Orchestrator parse error",
                    "orchestrator parsing",
                )));
            }
        };

        // Resolve wall-clock budget: DB-backed value takes priority over the
        // env-var / compiled-in DB-less fallback (Step 9.3 demotion).
        let effective_duration = max_duration_override.unwrap_or_else(orchestrator_max_duration);
        let effective_limits = ResourceLimits::new()
            .max_duration(effective_duration)
            .max_allocations(ORCHESTRATOR_MAX_ALLOCATIONS)
            .max_memory(128 * 1024 * 1024); // 128 MB

        // Start execution
        let mut stdout = String::new();
        let tracker = LimitedTracker::new(effective_limits);

        let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runner.start(
                input_values,
                tracker,
                PrintWriter::CollectString(&mut stdout),
            )
        }));

        let progress = match run_result {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                return Err(EngineError::Orchestrator(classify_orchestrator_failure(
                    "Orchestrator runtime error",
                    &e.to_string(),
                )));
            }
            Err(_) => {
                return Err(EngineError::Orchestrator(orchestrator_vm_panic(
                    "Orchestrator runtime error",
                    "orchestrator start",
                )));
            }
        };

        Ok(Self {
            progress: Some(progress),
            parked_call: None,
            total_tokens,
            final_result: None,
            stdout,
        })
    }

    /// Drive the session until it either completes or parks on
    /// `host.await_next_turn()`. When called after a park, `new_input` is fed
    /// to the suspended `await_next_turn()` call as its return value before
    /// continuing the dispatch loop. All host-call handler arms are identical
    /// to [`execute_orchestrator`]; only the accumulated state
    /// (`progress`/`total_tokens`/`final_result`/`stdout`) lives on `self` so
    /// it survives across turns (C.6 D-C1 cross-turn persistence).
    #[allow(clippy::too_many_arguments)]
    pub async fn drive_to_yield(
        &mut self,
        thread: &mut Thread,
        effects: &Arc<dyn EffectExecutor>,
        leases: &Arc<LeaseManager>,
        policy: &Arc<PolicyEngine>,
        signal_rx: &mut SignalReceiver,
        event_tx: Option<&tokio::sync::broadcast::Sender<ThreadEvent>>,
        retrieval: Option<&RetrievalEngine>,
        store: Option<&Arc<dyn Store>>,
        _platform_info: Option<&crate::executor::prompt::PlatformInfo>,
        gate_controller: &Arc<dyn crate::gate::GateController>,
        _retrieval_source: Option<&Arc<dyn RetrievalSource>>,
        dynamic_tools: Option<&Arc<dyn DynamicToolPort>>,
        component_port: Option<&Arc<dyn crate::executor::ComponentPort>>,
        kohai_port: Option<&Arc<dyn crate::executor::KohaiPort>>,
        new_input: Option<MontyObject>,
    ) -> Result<OrchestratorYield, EngineError> {
        // If we parked on host.await_next_turn() last drive, resume the
        // suspended call with the new turn's input before continuing the
        // dispatch loop.
        if let Some(call) = self.parked_call.take() {
            let ext_result = ExtFunctionResult::Return(new_input.unwrap_or(MontyObject::None));
            self.progress = Some(
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    call.resume(ext_result, PrintWriter::CollectString(&mut self.stdout))
                })) {
                    Ok(Ok(p)) => p,
                    Ok(Err(e)) => {
                        return Err(EngineError::Orchestrator(classify_orchestrator_failure(
                            "Orchestrator error after await_next_turn resume",
                            &e.to_string(),
                        )));
                    }
                    Err(_) => {
                        return Err(EngineError::Orchestrator(orchestrator_vm_panic(
                            "Orchestrator error after await_next_turn resume",
                            "orchestrator resume",
                        )));
                    }
                },
            );
        }

        loop {
            let progress = match self.progress.take() {
                Some(p) => p,
                None => {
                    return Err(EngineError::Orchestrator(classify_orchestrator_failure(
                        "Orchestrator session driven with no pending progress",
                        "drive_to_yield on a completed/empty session",
                    )));
                }
            };
            match progress {
                RunProgress::Complete(obj) => {
                    // Use FINAL result if set, otherwise fall back to VM return value
                    let result = if let Some(ref fr) = self.final_result {
                        fr.clone()
                    } else {
                        monty_to_json(&obj)
                    };
                    sync_runtime_state(thread, result.get("state"));
                    let outcome = parse_outcome(&result);
                    sync_visible_outcome(thread, &outcome);
                    normalize_pause_outcome(thread, &outcome)?;
                    let tier_zero_outcome = build_tier_zero_outcome(&result, &thread.events);
                    return Ok(OrchestratorYield::Complete(Box::new(OrchestratorResult {
                        outcome,
                        tokens_used: self.total_tokens,
                        tier_zero_outcome,
                    })));
                }

                RunProgress::FunctionCall(call) => {
                    let action_name = call.function_name.clone();
                    // Park: host.await_next_turn() suspends the VM until the
                    // next turn's input arrives. Retain the suspended call so
                    // the session can be resumed later (true cross-turn
                    // persistence). The non-persistent caller treats this as
                    // an error; the persistent driver parks the session.
                    if call.method_call && action_name == "await_next_turn" {
                        debug!("orchestrator: host.await_next_turn() - parking session");
                        self.parked_call = Some(call);
                        return Ok(OrchestratorYield::AwaitNextTurn);
                    }
                    let args = &call.args;
                    let kwargs = &call.kwargs;

                    debug!(action = %action_name, "orchestrator: host function call");

                    let ext_result = match action_name.as_str() {
                        // FINAL(result) — orchestrator returns its outcome
                        "FINAL" => {
                            let val = args.first().map(monty_to_json).unwrap_or_default();
                            self.final_result = Some(val);
                            ExtFunctionResult::Return(MontyObject::None)
                        }

                        // __check_signals__()
                        "__check_signals__" => handle_check_signals(signal_rx, thread),

                        // __emit_event__(kind, **data)
                        "__emit_event__" => handle_emit_event(args, kwargs, thread, event_tx),

                        // __save_checkpoint__(state, counters)
                        "__save_checkpoint__" => handle_save_checkpoint(args, kwargs, thread),

                        // __transition_to__(state, reason)
                        "__transition_to__" => handle_transition_to(args, kwargs, thread),

                        // __retrieve_docs__(goal, max_docs)
                        "__retrieve_docs__" => {
                            handle_retrieve_docs(args, kwargs, thread, retrieval).await
                        }

                        // __check_budget__()"
                        "__check_budget__" => handle_check_budget(thread),

                        // __log_budget_warning__(field, value, message)
                        // Soft telemetry — emits a BudgetWarning event but does not
                        // abort the orchestrator. Token-budget soft warnings are the
                        // only soft signal: time/cost budgets remain hard-stops.
                        "__log_budget_warning__" => {
                            handle_log_budget_warning(args, kwargs, thread, event_tx)
                        }

                        // __get_reduction_rules__() -> list
                        // Returns the per-project/user cached reduction rules used
                        // by the segment reduction pipeline in default.py.
                        "__get_reduction_rules__" => {
                            handle_get_reduction_rules(thread, store).await
                        }

                        // __get_actions__()
                        "__get_actions__" => {
                            handle_get_actions(thread, effects, leases, store).await
                        }

                        // __list_skills__(max_candidates, max_tokens)
                        "__list_skills__" => handle_list_skills(args, thread, component_port).await,

                        // __record_skill_usage__(doc_id, success)
                        "__record_skill_usage__" => handle_record_skill_usage(args, store).await,

                        // __regex_match__(pattern, text) -> bool
                        // Evaluates a regex against text using Rust's regex crate.
                        // Invalid patterns return False silently. Monty has no `re`
                        // module, so this host function bridges the gap for the
                        // skill selector's pattern-based scoring.
                        "__regex_match__" => handle_regex_match(args),

                        // __validate_component__(title, content, doc_type, metadata)
                        // Intercepts self-improvement memory_write calls for protected
                        // components (orchestrator:main, prompt:codeact_preamble).
                        // Creates an update-candidate MemoryDoc in Q1 (pending) instead
                        // of writing directly. Spec §3.5 / §3.6.
                        "__validate_component__" => {
                            handle_validate_component(args, thread, store).await
                        }

                        // __fetch_component__(uuid, class_code) -> dict | None
                        // Fetches a single validated component by UUID + class code from
                        // its class-specific table (SEC-01 gate). Used by `call_action`
                        // nested lookups (plan §0.9); Phase G depends on it. v3 Phase F.6
                        // (Q-F3).
                        "__fetch_component__" => {
                            handle_fetch_component(args, thread, component_port).await
                        }

                        // __resolve_component_by_name__(name, class_code) -> dict | None
                        // The §0.9 Option B fallback: fetches a single validated
                        // component by name + class code (SEC-01 gate). Used by
                        // `call_action` when it holds a step name, not a UUID.
                        // v3 Phase G.2 (Q-G4).
                        "__resolve_component_by_name__" => {
                            handle_resolve_component_by_name(args, thread, component_port).await
                        }

                        // ── C.1 first-class `host.*` callables ───────────────────────
                        // `host.<tool>(...)` surfaces as a MethodCall: function_name is the
                        // bare tool name, method_call is true, and args[0] is the `host`
                        // Dataclass (self). Skip args[0]; kwargs are untouched. These arms
                        // reuse the existing Rust handlers verbatim — wiring, not new logic.
                        // Net-new handlers land inline as they are implemented: resolve_intent
                        // (Phase 2) + post_reply (end-of-turn chat post) are done;
                        // kohai_complete follows next; compose_orchestrator's rewrite lands
                        // with the Recipe/Component rework in a later C substep.
                        "resolve_intent" if call.method_call => {
                            handle_resolve_intent(&args[1..], kwargs, thread, component_port).await
                        }
                        "post_reply" if call.method_call => {
                            handle_post_reply(&args[1..], kwargs, thread, event_tx)
                        }
                        "fetch_component" if call.method_call => {
                            handle_fetch_component(&args[1..], thread, component_port).await
                        }
                        "resolve_component_by_name" if call.method_call => {
                            handle_resolve_component_by_name(&args[1..], thread, component_port)
                                .await
                        }
                        "validate_component" if call.method_call => {
                            handle_validate_component(&args[1..], thread, store).await
                        }
                        "check_signals" if call.method_call => {
                            handle_check_signals(signal_rx, thread)
                        }
                        // Reused existing tools exposed under the `host.*` namespace.
                        "regex_match" if call.method_call => handle_regex_match(&args[1..]),
                        "skill_list" if call.method_call => {
                            handle_list_skills(&args[1..], thread, component_port).await
                        }

                        // C.4.5.17: host.run_program(code) — run a dynamically-provided
                        // Python code string via a NESTED execute_code (fresh
                        // isolation per call — mirrors execute_tier_zero_channel).
                        // Monty iterates composed.steplist and calls this once per
                        // step's executable_code. Returns {ok, return_value, stdout,
                        // error}.
                        "run_program" if call.method_call => {
                            handle_run_program(
                                &args[1..],
                                thread,
                                effects,
                                leases,
                                policy,
                                gate_controller,
                                event_tx,
                            )
                            .await
                        }

                        // C.4.5.17: compose a recipe (component_id) + variant
                        // (step_link) into the predefined ComposedProgram via the
                        // composition-system port (the IBS). Monty iterates the
                        // returned steplist, consults the skills array for exact
                        // tool usage, and runs each step's executable_code via
                        // host.run_program. The cdylib application of
                        // rust_directives is a C.5/C.6 concern (deferred). Returns
                        // {ok, program} on success; {ok:false, error} on no bridge
                        // / not found / failure.
                        "compose_orchestrator" if call.method_call => {
                            handle_compose_orchestrator(&args[1..], thread, component_port).await
                        }

                        // C.5: host.kohai_complete(prompt={chat_history, user_query,
                        // prefix_placeholder}) — the Orchestrator→Kohai LLM handoff.
                        // Runs the full interceptor ingress (forensic-packet capture
                        // → optional Sempai → provider-prefix swap → provider gateway
                        // call → packet close) via the composition-side port. Returns
                        // {ok, answer, usage} on success; {ok:false, error} on no
                        // bridge / invalid prompt / failure. Monty drives this; Rust
                        // is the host.
                        "kohai_complete" if call.method_call => {
                            handle_kohai_complete(&args[1..], kwargs, thread, kohai_port).await
                        }

                        // ── C.3 dynamic cdylib Tool fallthrough ─────────────────────
                        // A `host.<name>(...)` call whose name is not a built-in. If a
                        // dynamic Tool is loaded under that name, route the call through
                        // the DynamicToolPort (JSON-in/JSON-out); otherwise let Monty
                        // resolve it (user-defined functions, builtins). The impl lives in
                        // composition (C.5/C.6) over `DynamicToolLoader`; until then
                        // `dynamic_tools` is `None` and this arm is dormant.
                        other if call.method_call => match dynamic_tools {
                            Some(port) => dispatch_dynamic_tool(&**port, other, &args[1..], kwargs),
                            None => ExtFunctionResult::NotFound(other.to_string()),
                        },

                        // Unknown — let Monty resolve it (user-defined functions, builtins)
                        other => ExtFunctionResult::NotFound(other.to_string()),
                    };

                    // Resume the orchestrator VM
                    self.progress = Some(
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            call.resume(ext_result, PrintWriter::CollectString(&mut self.stdout))
                        })) {
                            Ok(Ok(p)) => p,
                            Ok(Err(e)) => {
                                return Err(EngineError::Orchestrator(
                                    classify_orchestrator_failure(
                                        "Orchestrator error after resume",
                                        &e.to_string(),
                                    ),
                                ));
                            }
                            Err(_) => {
                                return Err(EngineError::Orchestrator(orchestrator_vm_panic(
                                    "Orchestrator error after resume",
                                    "orchestrator resume",
                                )));
                            }
                        },
                    );

                    // If FINAL was called, the VM should complete on next iteration
                    if self.final_result.is_some() {
                        continue;
                    }
                }

                RunProgress::NameLookup(lookup) => {
                    let name = lookup.name.clone();
                    // C.1: the Monty namespace IS the tool registry. The `host` object is a
                    // frozen Dataclass with empty attrs. `host.<tool>(...)` compiles to
                    // `CallAttr`; the dataclass `py_call_attr` routes any public attr that is
                    // not in attrs to `MethodCall`, which surfaces as
                    // `FunctionCall{function_name:"<tool>", method_call:true, args[0]=self}`.
                    // The host dispatches on the bare tool name (skipping args[0]); kwargs are
                    // untouched. Storing `Function` values in attrs would raise TypeError on
                    // CallAttr, so the namespace deliberately carries no attrs.
                    let result = if name == "host" {
                        debug!(name = %name, "orchestrator: resolved host namespace");
                        NameLookupResult::Value(MontyObject::Dataclass {
                            name: "host".to_string(),
                            type_id: HOST_NAMESPACE_TYPE_ID,
                            field_names: Vec::new(),
                            attrs: DictPairs::from(Vec::new()),
                            frozen: true,
                        })
                    } else {
                        debug!(name = %name, "orchestrator: unresolved name");
                        NameLookupResult::Undefined
                    };
                    self.progress = Some(
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            lookup.resume(result, PrintWriter::CollectString(&mut self.stdout))
                        })) {
                            Ok(Ok(p)) => p,
                            Ok(Err(e)) => {
                                return Err(EngineError::Orchestrator(
                                    classify_orchestrator_failure(
                                        &format!("Orchestrator NameError '{name}'"),
                                        &e.to_string(),
                                    ),
                                ));
                            }
                            Err(_) => {
                                return Err(EngineError::Orchestrator(orchestrator_vm_panic(
                                    &format!("Orchestrator NameError '{name}'"),
                                    "name lookup",
                                )));
                            }
                        },
                    );
                }

                RunProgress::OsCall(_) => {
                    return Err(EngineError::Effect {
                        reason: "Orchestrator attempted OS call (blocked)".into(),
                    });
                }

                RunProgress::ResolveFutures(_) => {
                    return Err(EngineError::Effect {
                        reason: "Orchestrator attempted async (not supported)".into(),
                    });
                }
            }
        }
    }
}

/// Construct a fresh cross-turn-persistent [`MontySession`] for one
/// conversation (C.6 slice 4a). This is the *fresh-session* half of the
/// persistent-Monty turn bootstrap: it loads the versioned orchestrator code
/// and the runtime checkpoint's `persisted_state`, then parses + starts the
/// script up to the first host call via [`MontySession::new`].
///
/// The per-turn half (load the `Thread` from the store, transition it to
/// `Running`, hand the new turn's input to a checked-out session via
/// [`MontySession::drive_to_yield`]) is owned by the composition-side
/// `PersistentMontyDriver`, which calls this inside the session registry's
/// `checkout_or_create` init closure so a session is built only when no parked
/// session exists for the conversation.
///
/// Deliberately minimal vs the retired `ExecutionLoop::run` bootstrap: the
/// orchestrator (`basic_mode.py`) assembles every LLM prompt itself via
/// `host.*` and persists history via `host.save_history`, so the Model-A
/// `refresh_system_prompt` / `persist_runtime_state` / `store_runtime_checkpoint`
/// steps are NOT replicated here.
pub async fn prepare_monty_session(
    thread: &Thread,
    store: Option<&Arc<dyn Store>>,
    max_duration_override: Option<std::time::Duration>,
) -> Result<MontySession, EngineError> {
    // Pre-fetch shared memory docs — only consulted when self-modification is
    // enabled; otherwise load_orchestrator_from_docs returns the compiled-in
    // DEFAULT_ORCHESTRATOR and the docs are unused.
    let system_docs = match store {
        Some(store) => match store.list_shared_memory_docs(thread.project_id).await {
            Ok(docs) => docs,
            Err(error) => {
                debug!("failed to load shared docs for orchestrator: {error}");
                Vec::new()
            }
        },
        None => Vec::new(),
    };

    let allow_self_modify = crate::runtime::self_modify_enabled();
    let (orchestrator_code, _orchestrator_version) =
        load_orchestrator_from_docs(&system_docs, allow_self_modify);

    let persisted_state = thread
        .metadata
        .get(crate::runtime::manager::RUNTIME_CHECKPOINT_METADATA_KEY)
        .and_then(|value| value.get("persisted_state"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    MontySession::new(
        &orchestrator_code,
        thread,
        &persisted_state,
        max_duration_override,
    )
}

/// Drive an orchestrator script once to completion (non-persistent path).
///
/// Thin delegation over [`MontySession`]: parse + start, then drive until the
/// script completes. If the script calls `host.await_next_turn()` (which only
/// the cross-turn-persistent driver in C.6 slice 4 handles), this returns an
/// [`EngineError::Orchestrator`] — the non-persistent path cannot park a VM
/// across turns.
#[allow(clippy::too_many_arguments)]
pub async fn execute_orchestrator(
    code: &str,
    thread: &mut Thread,
    effects: &Arc<dyn EffectExecutor>,
    leases: &Arc<LeaseManager>,
    policy: &Arc<PolicyEngine>,
    signal_rx: &mut SignalReceiver,
    event_tx: Option<&tokio::sync::broadcast::Sender<ThreadEvent>>,
    retrieval: Option<&RetrievalEngine>,
    store: Option<&Arc<dyn Store>>,
    platform_info: Option<&crate::executor::prompt::PlatformInfo>,
    gate_controller: &Arc<dyn crate::gate::GateController>,
    persisted_state: &serde_json::Value,
    _retrieval_source: Option<&Arc<dyn RetrievalSource>>,
    dynamic_tools: Option<&Arc<dyn DynamicToolPort>>,
    component_port: Option<&Arc<dyn crate::executor::ComponentPort>>,
    kohai_port: Option<&Arc<dyn crate::executor::KohaiPort>>,
    max_duration_override: Option<std::time::Duration>,
) -> Result<OrchestratorResult, EngineError> {
    let mut session = MontySession::new(code, thread, persisted_state, max_duration_override)?;
    match session
        .drive_to_yield(
            thread,
            effects,
            leases,
            policy,
            signal_rx,
            event_tx,
            retrieval,
            store,
            platform_info,
            gate_controller,
            _retrieval_source,
            dynamic_tools,
            component_port,
            kohai_port,
            None,
        )
        .await?
    {
        OrchestratorYield::Complete(result) => Ok(*result),
        OrchestratorYield::AwaitNextTurn => {
            Err(EngineError::Orchestrator(classify_orchestrator_failure(
                "Orchestrator parked awaiting next turn",
                "host.await_next_turn() in non-persistent mode",
            )))
        }
    }
}

// ── Host function handlers ──────────────────────────────────

/// Handle `__check_signals__()`.
fn handle_check_signals(signal_rx: &mut SignalReceiver, thread: &mut Thread) -> ExtFunctionResult {
    match signal_rx.try_recv() {
        Ok(ThreadSignal::Stop) | Ok(ThreadSignal::Suspend) => {
            ExtFunctionResult::Return(MontyObject::String("stop".into()))
        }
        Ok(ThreadSignal::InjectMessage(msg)) => {
            thread.add_message(msg.clone());
            let result = serde_json::json!({"inject": msg.content});
            ExtFunctionResult::Return(json_to_monty(&result))
        }
        Ok(ThreadSignal::Resume) | Ok(ThreadSignal::ChildCompleted { .. }) => {
            ExtFunctionResult::Return(MontyObject::None)
        }
        Err(_) => ExtFunctionResult::Return(MontyObject::None),
    }
}

/// Handle `__emit_event__(kind, **data)`.
fn handle_emit_event(
    args: &[MontyObject],
    kwargs: &[(MontyObject, MontyObject)],
    thread: &mut Thread,
    event_tx: Option<&tokio::sync::broadcast::Sender<ThreadEvent>>,
) -> ExtFunctionResult {
    let kind_str = args.first().map(monty_to_string).unwrap_or_default();

    let kind = match kind_str.as_str() {
        "step_started" => {
            let _step = extract_u64_kwarg(kwargs, "step").unwrap_or(0);
            EventKind::StepStarted {
                step_id: StepId::new(),
            }
        }
        "step_completed" => {
            let input = extract_u64_kwarg(kwargs, "input_tokens").unwrap_or(0);
            let output = extract_u64_kwarg(kwargs, "output_tokens").unwrap_or(0);
            // Increment step count (mirrors the old Rust loop's step_count += 1)
            thread.step_count += 1;
            // Track token usage
            thread.total_tokens_used += input + output;
            EventKind::StepCompleted {
                step_id: StepId::new(),
                tokens: TokenUsage {
                    input_tokens: input,
                    output_tokens: output,
                    ..Default::default()
                },
            }
        }
        "action_executed" => {
            let action_name = extract_string_kwarg(kwargs, "action_name").unwrap_or_default();
            let call_id = extract_string_kwarg(kwargs, "call_id").unwrap_or_default();
            EventKind::ActionExecuted {
                step_id: StepId::new(),
                action_name,
                call_id,
                duration_ms: 0,
                params_summary: None,
            }
        }
        "action_failed" => {
            let action_name = extract_string_kwarg(kwargs, "action_name").unwrap_or_default();
            let call_id = extract_string_kwarg(kwargs, "call_id").unwrap_or_default();
            let error = extract_string_kwarg(kwargs, "error").unwrap_or_default();
            let duration_ms = extract_u64_kwarg(kwargs, "duration_ms").unwrap_or(0);
            EventKind::ActionFailed {
                step_id: StepId::new(),
                action_name,
                call_id,
                error,
                duration_ms,
                params_summary: None,
            }
        }
        "budget_warning" => {
            let field = extract_string_kwarg(kwargs, "field").unwrap_or_default();
            let value = extract_i64_kwarg(kwargs, "value").unwrap_or(0);
            let message = extract_string_kwarg(kwargs, "message").unwrap_or_default();
            EventKind::BudgetWarning {
                field,
                value,
                message,
            }
        }
        "prompt_over_budget" => {
            let estimated = extract_u64_kwarg(kwargs, "estimated_tokens").unwrap_or(0);
            let budget = extract_u64_kwarg(kwargs, "budget_tokens").unwrap_or(0);
            EventKind::PromptOverBudget {
                estimated_tokens: estimated,
                budget_tokens: budget,
            }
        }
        // ── Recipe Tier-0 execution (v3 Phase H.4) ───────────────
        // Reused by Model B/C: emitted by the agent-loop Tier-0 path (the
        // `LoopOrchestratorPort` driver + the engine `pub` fns extracted in
        // H.8). The Model A `default.py` step-0 `tier_zero` branch that
        // previously emitted these was removed in v3 Phase H.5 O3.
        // Before H4.2 the fallthrough below DROPPED them (`debug!` + return,
        // NOT pushed to `thread.events`), so a Tier-0 run left no record and
        // the composition listener (H4.7) could never see the outcome. Now
        // they are typed `EventKind` variants pushed to `thread.events` +
        // broadcast on `event_tx`. `recipe_id` rides the `recipe_id` kwarg
        // (H4.5 adds it); `recipe_name` rides the `recipe` kwarg (the name
        // the emitter passes as `recipe=...` — kwarg shape preserved from
        // the removed H.3 branch); `message` (failed only) rides the
        // `message` kwarg.
        "recipe_tier_zero_started" => {
            let recipe_id = extract_string_kwarg(kwargs, "recipe_id").unwrap_or_default();
            let recipe_name = extract_string_kwarg(kwargs, "recipe").unwrap_or_default();
            EventKind::RecipeTierZeroStarted {
                recipe_id,
                recipe_name,
            }
        }
        "recipe_tier_zero_succeeded" => {
            let recipe_id = extract_string_kwarg(kwargs, "recipe_id").unwrap_or_default();
            let recipe_name = extract_string_kwarg(kwargs, "recipe").unwrap_or_default();
            EventKind::RecipeTierZeroSucceeded {
                recipe_id,
                recipe_name,
            }
        }
        "recipe_tier_zero_failed" => {
            let recipe_id = extract_string_kwarg(kwargs, "recipe_id").unwrap_or_default();
            let recipe_name = extract_string_kwarg(kwargs, "recipe").unwrap_or_default();
            let message = extract_string_kwarg(kwargs, "message").unwrap_or_default();
            EventKind::RecipeTierZeroFailed {
                recipe_id,
                recipe_name,
                message,
            }
        }
        _ => {
            debug!(kind = %kind_str, "orchestrator: unknown event kind, skipping");
            return ExtFunctionResult::Return(MontyObject::None);
        }
    };

    let event = ThreadEvent::new(thread.id, kind);
    if let Some(tx) = event_tx {
        let _ = tx.send(event.clone());
    }
    thread.events.push(event);
    thread.updated_at = chrono::Utc::now();

    ExtFunctionResult::Return(MontyObject::None)
}

/// Handle `host.post_reply(text=...)` — end-of-turn answer post. Per the
/// locked architecture (A1) only Rust owns the chat socket, so the Orchestrator
/// hands its final answer to this Tool: it appends an Assistant message to the
/// thread transcript and emits a `MessageAdded` event (the chat-window surface).
/// Wiring over the existing `ThreadMessage::assistant` + `ThreadEvent` path —
/// no new logic.
fn handle_post_reply(
    args: &[MontyObject],
    kwargs: &[(MontyObject, MontyObject)],
    thread: &mut Thread,
    event_tx: Option<&tokio::sync::broadcast::Sender<ThreadEvent>>,
) -> ExtFunctionResult {
    let text = extract_string_arg(args, kwargs, "text", 0).unwrap_or_default();
    if text.is_empty() {
        return ExtFunctionResult::Return(MontyObject::None);
    }
    thread.messages.push(ThreadMessage::assistant(&text));
    let preview: String = text.chars().take(200).collect();
    let event = ThreadEvent::new(
        thread.id,
        EventKind::MessageAdded {
            role: "assistant".to_string(),
            content_preview: preview,
        },
    );
    if let Some(tx) = event_tx {
        let _ = tx.send(event.clone());
    }
    thread.events.push(event);
    thread.updated_at = chrono::Utc::now();
    ExtFunctionResult::Return(MontyObject::None)
}

/// Handle `__save_checkpoint__(state, counters)`.
fn handle_save_checkpoint(
    args: &[MontyObject],
    _kwargs: &[(MontyObject, MontyObject)],
    thread: &mut Thread,
) -> ExtFunctionResult {
    let state = args
        .first()
        .map(monty_to_json)
        .unwrap_or(serde_json::json!({}));
    let counters = args
        .get(1)
        .map(monty_to_json)
        .unwrap_or(serde_json::json!({}));

    sync_runtime_state(thread, Some(&state));

    if let Some(metadata) = thread.metadata.as_object_mut() {
        metadata.insert(
            "runtime_checkpoint".into(),
            serde_json::json!({
                "persisted_state": state,
                "nudge_count": counters.get("nudge_count").and_then(|v| v.as_u64()).unwrap_or(0),
                "consecutive_errors": counters.get("consecutive_errors").and_then(|v| v.as_u64()).unwrap_or(0),
                "consecutive_action_errors": counters.get("consecutive_action_errors").and_then(|v| v.as_u64()).unwrap_or(0),
                "compaction_count": counters.get("compaction_count").and_then(|v| v.as_u64()).unwrap_or(0),
            }),
        );
    }
    thread.updated_at = chrono::Utc::now();

    ExtFunctionResult::Return(MontyObject::None)
}

/// Handle `__transition_to__(state, reason)`.
fn handle_transition_to(
    args: &[MontyObject],
    _kwargs: &[(MontyObject, MontyObject)],
    thread: &mut Thread,
) -> ExtFunctionResult {
    let state_str = args.first().map(monty_to_string).unwrap_or_default();
    let reason = args.get(1).map(monty_to_string);

    let target = match state_str.as_str() {
        "running" => crate::types::thread::ThreadState::Running,
        "completed" => crate::types::thread::ThreadState::Completed,
        "failed" => crate::types::thread::ThreadState::Failed,
        "waiting" => crate::types::thread::ThreadState::Waiting,
        "suspended" => crate::types::thread::ThreadState::Suspended,
        other => {
            return ExtFunctionResult::Error(monty::MontyException::new(
                monty::ExcType::ValueError,
                Some(format!("Unknown thread state: {other}")),
            ));
        }
    };

    match thread.transition_to(target, reason) {
        Ok(()) => ExtFunctionResult::Return(MontyObject::None),
        Err(e) => ExtFunctionResult::Error(monty::MontyException::new(
            monty::ExcType::RuntimeError,
            Some(format!("State transition failed: {e}")),
        )),
    }
}

/// Handle `__retrieve_docs__(goal, max_docs)`.
async fn handle_retrieve_docs(
    args: &[MontyObject],
    _kwargs: &[(MontyObject, MontyObject)],
    thread: &Thread,
    retrieval: Option<&RetrievalEngine>,
) -> ExtFunctionResult {
    let retrieval = match retrieval {
        Some(r) => r,
        None => return ExtFunctionResult::Return(json_to_monty(&serde_json::json!([]))),
    };

    let goal = args.first().map(monty_to_string).unwrap_or_default();
    let max_docs = args
        .get(1)
        .and_then(|v| match v {
            MontyObject::Int(i) => Some(*i as usize),
            _ => None,
        })
        .unwrap_or(5);

    match retrieval
        .retrieve_context(thread.project_id, &thread.user_id, &goal, max_docs)
        .await
    {
        Ok(docs) => {
            let docs_json: Vec<serde_json::Value> = docs
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "type": format!("{:?}", d.doc_type),
                        "title": d.title,
                        "content": d.content,
                    })
                })
                .collect();
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!(docs_json)))
        }
        Err(e) => {
            debug!("retrieve_docs failed: {e}");
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!([])))
        }
    }
}

/// Handle `__fetch_component__(uuid, class_code)` (v3 Phase F.6 / Q-F3).
///
/// Thin-calls [`ComponentPort::fetch_component`] (the composition-side impl runs
/// the SEC-01-validated `fetch_component_by_id` class-table SELECT). Used by
/// `call_action` nested lookups (plan §0.9); Phase G depends on it.
///
/// Returns a Python dict `{ id, class_code, name, description, content,
/// override_prompt_creation }` (+ `steps` / `allowed_tools` for class-16
/// Actions) for the single matched [`ComponentItem`], or `None` when: no bridge
/// is wired (`None` port, e.g. non-skills-db config / unit-test path), the UUID
/// or class-code args are missing/invalid, the component is absent, or the fetch
/// errors. The retrieval scope is built from the thread's real identity
/// (`thread.tenant_id` / `thread.agent_id` — F.1/F.3); the LIVE agent-loop path
/// sources tenant from `LoopRunContext.scope.tenant_id` (F.4).
async fn handle_fetch_component(
    _args: &[MontyObject],
    _thread: &Thread,
    component_port: Option<&Arc<dyn crate::executor::ComponentPort>>,
) -> ExtFunctionResult {
    let uuid_str = _args.first().map(monty_to_string).unwrap_or_default();
    let Ok(component_id) = uuid::Uuid::parse_str(&uuid_str) else {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null));
    };
    let class_code = match _args.get(1) {
        Some(MontyObject::Int(i)) => *i as i32,
        _ => {
            return ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null));
        }
    };
    let Some(port) = component_port else {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null));
    };

    // Build the retrieval scope from the thread's real identity. v3 Phase F
    // (Q-F2 / FIND-P8-01): mirrors the F.3-fixed `handle_assemble_prior_
    // knowledge` scope. The LIVE retrieval path sources tenant from
    // `LoopRunContext.scope.tenant_id` (F.4); this dormant engine handler
    // reads `thread.tenant_id` / `thread.agent_id` so it is correct if
    // re-activated.
    let scope = ComponentScope {
        tenant_id: _thread.tenant_id.clone(),
        user_id: _thread.user_id.clone(),
        agent_id: _thread.agent_id.clone(),
        project_id: _thread.project_id.to_string(),
    };

    match port.fetch_component(&scope, component_id, class_code).await {
        Ok(Some(item)) => {
            let mut value = serde_json::json!({
                "id": item.id.to_string(),
                "class_code": item.class_code,
                "name": item.name,
                "description": item.description,
                "content": item.effective_content,
                "override_prompt_creation": item.override_prompt_creation,
            });
            // Q-G-STUB1: surface the executable `steps` + `allowed_tools`
            // for class-16 Actions so `execute_action_procedure` can run
            // the real procedure (absent for every other class).
            if let Some(obj) = value.as_object_mut() {
                if let Some(steps) = item.steps {
                    obj.insert("steps".to_string(), steps);
                }
                if let Some(allowed_tools) = item.allowed_tools {
                    obj.insert("allowed_tools".to_string(), allowed_tools);
                }
            }
            ExtFunctionResult::Return(json_to_monty(&value))
        }
        Ok(None) => {
            debug!("__fetch_component__: no validated component for uuid {component_id}");
            ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null))
        }
        Err(e) => {
            debug!("__fetch_component__: fetch failed: {e}");
            ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null))
        }
    }
}

/// Handle `host.run_program(code)` (C.4.5.17). Runs a dynamically-provided
/// Python code string via a NESTED [`execute_code`] — a fresh
/// [`ThreadExecutionContext`] + `persisted_state = {}` per call (ISOLATION
/// invariant; mirrors `execute_tier_zero_channel`). Monty iterates
/// `composed.steplist` and calls this once per step's `executable_code`; the
/// orchestrator driver (e.g. `default.py`) consumes the returned dict to decide
/// whether to continue to the next step.
///
/// Returns a Monty dict `{ok, return_value, stdout, error}`:
/// - `ok=true` + `return_value` (the code's Python return value) + `stdout` on
///   success (`error` is `null`).
/// - `ok=false` + `error` when `execute_code` errors, the result carries a
///   classified `failure`, or execution paused on an approval gate
///   (`error="approval_required"`).
#[allow(clippy::too_many_arguments)]
async fn handle_run_program(
    args: &[MontyObject],
    thread: &Thread,
    effects: &Arc<dyn EffectExecutor>,
    leases: &Arc<LeaseManager>,
    policy: &Arc<PolicyEngine>,
    gate_controller: &Arc<dyn crate::gate::GateController>,
    event_tx: Option<&tokio::sync::broadcast::Sender<ThreadEvent>>,
) -> ExtFunctionResult {
    let code = args.first().map(monty_to_string).unwrap_or_default();
    let exec_ctx = thread_execution_context(thread, StepId::new(), None, gate_controller.clone());
    let fresh_state = serde_json::json!({});

    match Box::pin(execute_code(
        &code,
        thread,
        None,
        effects,
        leases,
        policy,
        &exec_ctx,
        &[],
        &fresh_state,
    ))
    .await
    {
        Ok(result) => {
            for event_kind in &result.events {
                if let Some(tx) = event_tx {
                    let _ = tx.send(ThreadEvent::new(thread.id, event_kind.clone()));
                }
            }
            if let Some(failure) = result.failure {
                return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
                    "ok": false,
                    "return_value": serde_json::Value::Null,
                    "stdout": result.stdout,
                    "error": format!("{failure:?}"),
                })));
            }
            if result.need_approval.is_some() {
                return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
                    "ok": false,
                    "return_value": serde_json::Value::Null,
                    "stdout": result.stdout,
                    "error": "approval_required",
                })));
            }
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
                "ok": true,
                "return_value": result.return_value,
                "stdout": result.stdout,
                "error": serde_json::Value::Null,
            })))
        }
        Err(e) => ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": false,
            "return_value": serde_json::Value::Null,
            "stdout": "",
            "error": e.to_string(),
        }))),
    }
}

/// Handle `__resolve_component_by_name__(name, class_code)` (v3 Phase G.2 /
/// Q-G4 — the §0.9 Option B fallback host function).
///
/// Thin-calls [`ComponentPort::resolve_component_by_name`] (the composition-side
/// impl runs the SEC-01-validated `fetch_component_by_name` class-table SELECT).
/// Used by `call_action` when it holds a step **name** rather than a UUID
/// (plan §0.9 Option B).
///
/// Returns the same Python dict shape as `handle_fetch_component`
/// (`{ id, class_code, name, description, content, override_prompt_creation }`
/// and `steps` / `allowed_tools` for class-16 Actions) for the single matched
/// [`ComponentItem`], or `None` when no bridge is wired (`None` port, e.g. a
/// non-skills-db config or unit-test path), when the name or class-code args
/// are missing/invalid, when the component is absent, or when the fetch
/// errors. The scope is built from the thread's real identity
/// (`thread.tenant_id` / `thread.agent_id` — F.1/F.3), mirroring
/// `handle_fetch_component`.
async fn handle_resolve_component_by_name(
    _args: &[MontyObject],
    _thread: &Thread,
    component_port: Option<&Arc<dyn crate::executor::ComponentPort>>,
) -> ExtFunctionResult {
    let name = _args.first().map(monty_to_string).unwrap_or_default();
    if name.is_empty() {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null));
    }
    let class_code = match _args.get(1) {
        Some(MontyObject::Int(i)) => *i as i32,
        _ => {
            return ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null));
        }
    };
    let Some(port) = component_port else {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null));
    };

    let scope = ComponentScope {
        tenant_id: _thread.tenant_id.clone(),
        user_id: _thread.user_id.clone(),
        agent_id: _thread.agent_id.clone(),
        project_id: _thread.project_id.to_string(),
    };

    match port
        .resolve_component_by_name(&scope, &name, class_code)
        .await
    {
        Ok(Some(item)) => {
            let mut value = serde_json::json!({
                "id": item.id.to_string(),
                "class_code": item.class_code,
                "name": item.name,
                "description": item.description,
                "content": item.effective_content,
                "override_prompt_creation": item.override_prompt_creation,
            });
            // Q-G-STUB1: surface the executable `steps` + `allowed_tools`
            // for class-16 Actions (the §0.9 Option B fallback path),
            // mirroring `handle_fetch_component`.
            if let Some(obj) = value.as_object_mut() {
                if let Some(steps) = item.steps {
                    obj.insert("steps".to_string(), steps);
                }
                if let Some(allowed_tools) = item.allowed_tools {
                    obj.insert("allowed_tools".to_string(), allowed_tools);
                }
            }
            ExtFunctionResult::Return(json_to_monty(&value))
        }
        Ok(None) => {
            debug!("__resolve_component_by_name__: no validated component for name {name:?}");
            ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null))
        }
        Err(e) => {
            debug!("__resolve_component_by_name__: fetch failed: {e}");
            ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null))
        }
    }
}

/// Handle `host.compose_orchestrator(component_id, step_link, user_input)`
/// (C.4.5.17). Thin-calls the composition-system port (the IBS) to compose the
/// recipe (`component_id`) + variant (`step_link`, surfaced to Monty by
/// `host.resolve_intent`) into the predefined `ComposedProgram`, binding
/// `{{vars.NAME}}` slots captured from `user_input`. Returns a Monty dict:
///   `{"ok":true, "program":{skills,steplist,rust_directives,variables,
///                           assembled_program,tier}}`
///   `{"ok":false, "error":..}`  (no bridge / recipe not found / no variant
///                                match / composition failure)
/// Monty iterates `program.steplist`, consults `program.skills` for exact tool
/// usage, and runs each step's `executable_code` via `host.run_program`. The
/// cdylib *application* of `program.rust_directives` is a C.5/C.6 concern
/// (deferred — the directives are carried in the program for that wiring).
/// Scope is built from the thread's real identity, mirroring
/// `handle_fetch_component` / `handle_resolve_intent`.
async fn handle_compose_orchestrator(
    args: &[MontyObject],
    thread: &Thread,
    component_port: Option<&Arc<dyn crate::executor::ComponentPort>>,
) -> ExtFunctionResult {
    let component_id_str = args.first().map(monty_to_string).unwrap_or_default();
    let Ok(component_id) = uuid::Uuid::parse_str(&component_id_str) else {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": false,
            "error": "missing or invalid component_id",
        })));
    };
    let step_link = args.get(1).map(monty_to_string).unwrap_or_default();
    if step_link.is_empty() {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": false,
            "error": "missing step_link",
        })));
    }
    let user_input = args.get(2).map(monty_to_string).unwrap_or_default();
    let Some(port) = component_port else {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": false,
            "error": "composition_unavailable",
        })));
    };
    let scope = ComponentScope {
        tenant_id: thread.tenant_id.clone(),
        user_id: thread.user_id.clone(),
        agent_id: thread.agent_id.clone(),
        project_id: thread.project_id.to_string(),
    };
    match port
        .compose(&scope, component_id, &step_link, &user_input)
        .await
    {
        Ok(program) => {
            let program_value = serde_json::to_value(&program).unwrap_or(serde_json::Value::Null);
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
                "ok": true,
                "program": program_value,
            })))
        }
        Err(e) => ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": false,
            "error": e.to_string(),
        }))),
    }
}

/// Handle `host.kohai_complete(prompt=...)` (C.5) — the Orchestrator→Kohai LLM
/// handoff. Thin-calls [`crate::executor::KohaiPort::complete`], which runs the
/// full interceptor ingress (forensic-packet capture `[AwaitingKohai]` →
/// optional Sempai review → provider-prefix swap via `get_system_bundle` →
/// `HostManagedModelGateway::stream_model` → packet close `[Complete]`) in the
/// composition layer. Returns `{ok:true, answer, usage}` on success; `{ok:false,
/// error:"invalid prompt: …"}` when the `prompt` argument is missing/not a dict;
/// `{ok:false, error:"kohai_unavailable"}` when no bridge is wired. The
/// composition impl owns the provider gateway (the engine no longer threads an
/// `LlmBackend` into the port).
async fn handle_kohai_complete(
    args: &[MontyObject],
    kwargs: &[(MontyObject, MontyObject)],
    thread: &Thread,
    kohai_port: Option<&Arc<dyn crate::executor::KohaiPort>>,
) -> ExtFunctionResult {
    // `prompt` may be passed as a kwarg (`host.kohai_complete(prompt=…)`, the
    // seeded contract) or positionally (`host.kohai_complete(prompt)`).
    let prompt_value = kwargs
        .iter()
        .find_map(|(k, v)| match k {
            MontyObject::String(key) if key == "prompt" => Some(monty_to_json(v)),
            _ => None,
        })
        .or_else(|| args.first().map(monty_to_json));
    let Some(prompt) = prompt_value.filter(serde_json::Value::is_object) else {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": false,
            "error": "invalid prompt: missing or not a dict",
        })));
    };
    let Some(port) = kohai_port else {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": false,
            "error": "kohai_unavailable",
        })));
    };
    let ctx = crate::executor::kohai_port::KohaiCallCtx {
        run_id: thread.id.to_string(),
        iteration: thread.step_count as u32,
        user_id: thread.user_id.clone(),
        project_id: thread.project_id.to_string(),
        tenant_id: thread.tenant_id.clone(),
    };
    match port.complete(prompt, ctx).await {
        Ok(answer) => ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": true,
            "answer": answer.content,
            "usage": {
                "input_tokens": answer.usage.input_tokens,
                "output_tokens": answer.usage.output_tokens,
                "cost_usd": answer.usage.cost_usd,
            },
        }))),
        Err(e) => ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "ok": false,
            "error": e.to_string(),
        }))),
    }
}

/// Handle `host.resolve_intent(user_input=...)` — Phase 2 of the basic-mode
/// main process. The whole intent system is ONE Tool: this thin-calls the
/// [`ComponentPort::resolve_intent`] port (the composition-side impl wraps the
/// `intent_system::resolve_intent` SQL fn / `reborn_intent_inputs` lookup) —
/// wiring, not new logic. Returns a Python dict the orchestrator dispatches on:
///   `{"status":"match","component_id":..,"component_class_code":..,
///     "step_link":<str|null>,"component_name":..}`
///   `{"status":"disambiguation","candidates":[..]}`
///   `{"status":"no_match"}`
///   `{"status":"error","error":..}`
/// No bridge (`None` port, e.g. non-skills-db config / unit-test path) →
/// `no_match` (semantically correct — the run falls through to Non-Matching-Mode
/// / the LLM path). Scope is built from the thread's real identity
/// (`thread.tenant_id` / `thread.agent_id` — F.1/F.3), mirroring
/// `handle_fetch_component`.
async fn handle_resolve_intent(
    _args: &[MontyObject],
    _kwargs: &[(MontyObject, MontyObject)],
    _thread: &Thread,
    component_port: Option<&Arc<dyn crate::executor::ComponentPort>>,
) -> ExtFunctionResult {
    use crate::memory::intent_system::IntentResolution;

    let user_input = match extract_string_arg(_args, _kwargs, "user_input", 0) {
        Some(s) => s,
        None => {
            return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
                "status": "no_match",
                "error": "missing user_input",
            })));
        }
    };
    let Some(port) = component_port else {
        return ExtFunctionResult::Return(json_to_monty(&serde_json::json!({"status":"no_match"})));
    };
    let scope = ComponentScope {
        tenant_id: _thread.tenant_id.clone(),
        user_id: _thread.user_id.clone(),
        agent_id: _thread.agent_id.clone(),
        project_id: _thread.project_id.to_string(),
    };
    match port.resolve_intent(&scope, &user_input).await {
        Ok(IntentResolution::Match {
            component_id,
            component_class_code,
            step_link,
            component_name,
        }) => ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "status": "match",
            "component_id": component_id.to_string(),
            "component_class_code": component_class_code,
            "step_link": step_link,
            "component_name": component_name,
        }))),
        Ok(IntentResolution::Disambiguation { candidates }) => {
            let cands: Vec<serde_json::Value> = candidates
                .into_iter()
                .map(|c| {
                    serde_json::json!({
                        "component_id": c.component_id.to_string(),
                        "component_class_code": c.component_class_code,
                        "score": c.score,
                        "class_label": c.class_label,
                    })
                })
                .collect();
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
                "status": "disambiguation",
                "candidates": cands,
            })))
        }
        Ok(IntentResolution::NoMatch) => {
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!({"status":"no_match"})))
        }
        Err(e) => ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
            "status": "error",
            "error": e.to_string(),
        }))),
    }
}

/// The Capitalized category label for a component `class_code`, used as the
/// prose block heading in `orchestrator_content` (`## [{label}: {name}]`).
///
/// v3 Phase F.5 (Q-F7-2 / Q-F7-case): a thin helper over
/// [`crate::memory::instruction_builder::StepContextSpec::from_class_code`] +
/// [`StepContextSpec::heading`]. Returns `None` for class 13 (ToolSkill —
/// Rust-channel-only, §0.9 invariant) and class 11 (reserved), so the
/// formatter skips those items. This is NOT the lowercase specific subtype
/// from `class_label()` (e.g. `skill_rusty` / `extension_worker`); it is the
/// Capitalized category label the plan §0.9 StepContextSpec table specifies.
fn step_context_label(class_code: i32) -> Option<&'static str> {
    crate::memory::instruction_builder::StepContextSpec::from_class_code(class_code)
        .map(|spec| spec.heading())
}

/// Format the orchestrator-channel prior knowledge as a **prose**
/// StepContextSpec-headed block (plan §0.9 line 780–786).
///
/// Each [`ComponentItem`] becomes a `## [{heading}: {name}]\n{effective_content}`
/// block; blocks are joined by a blank line. Items whose `class_code` maps to
/// `None` (class 13 ToolSkill, class 11 reserved) are **skipped** — they never
/// appear in `orchestrator_content` (§0.9 invariant). An item with empty
/// `effective_content` emits a heading-only block (e.g. a Recipe with no body —
/// plan line 758). This is the v3 shape of `orchestrator_content` and
/// `formatted_content` (FINDING F: `formatted_content` transitions from a
/// JSON-encoded object to this prose string). Shared by
/// [`assemble_pkr_from_fetch`] (the `SplitResult` arm) and
/// [`assemble_pkr_from_items`] (the `Components` arm + the `recipe_hint`
/// Some-branch) so all three emit identical `orchestrator_content` for the
/// same items.
pub fn format_orchestrator_content(items: &[crate::memory::ComponentItem]) -> String {
    items
        .iter()
        .filter_map(|item| {
            let heading = step_context_label(item.class_code)?;
            let block = if item.effective_content.is_empty() {
                format!("## [{heading}: {}]", item.name)
            } else {
                format!("## [{heading}: {}]\n{}", item.name, item.effective_content)
            };
            Some(block)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ── Phase H.8 — extracted `pub` orchestrator fns for Model B/C ────────────
//
// The Phase-H agent-loop `RecipeStage` consumes prior knowledge + Tier-0
// channels via the composition `OrchestratorLookup` bridge (H.7). That bridge
// cannot call the private Monty handler `handle_assemble_prior_knowledge` or
// the Python `execute_recipe_orchestrator_channel` (FIND-NEW-PASS12-01/02), so
// H.8 extracts two `pub async fn`s (`assemble_prior_knowledge_with_hint` +
// `execute_tier_zero_channel`) returning these two `pub struct`s. The dormant
// Model A handler is deleted in H8.4 (user Q1=delete); these types back the
// Model B/C path only.

/// Result of Tier-1 prior-knowledge assembly
/// (`assemble_prior_knowledge_with_hint`, H8.2).
///
/// The **9-field reduced shape** (user Q2=reduced, plan-literal). The
/// `orchestrator_content` is the prose `## [Heading: name]\n<body>` block
/// produced by [`format_orchestrator_content`]; `matched_component_ids` is the
/// UUID identity set; `override_prompt_creation` flags the Solution-Override
/// path (§3.13 — exactly one override item → verbatim body).
///
/// **Routing is NOT carried here** (user Q2 lock): the Tier-0/Tier-1 routing
/// decision for Model B/C comes from `RetrievalTurnResult` (H.4 —
/// `tier0_eligible` / `llm_call_required` / `routing_meta.variant`), branched
/// on inline in `RecipeStage::process` (Phase H.10), NOT from this struct. The
/// matched `orchestrator_items` / `rust_items` are stashed on
/// `LoopExecutionState.recipe_hint` / `recipe_rust_context` (H.9). The
/// `action_short_circuit` / `action_component_id` / `action_name` /
/// `disambiguation` / `candidates` / `tier_zero` fields are therefore
/// **vestigial under Q2** — kept because the plan-literal 9 fields were locked.
/// The `None`-branch of `assemble_prior_knowledge_with_hint` (fresh
/// `fetch_for_turn`) populates them from the `FetchForTurnResult` arms so the
/// struct stays a faithful projection of the retrieval outcome; the
/// `Some`-branch (assemble stashed `Vec<ComponentItem>`) defaults them to the
/// non-short-circuit / non-disambiguation shape (Tier-1 assemble-only).
#[derive(Debug, Clone, PartialEq)]
pub struct PkrAssemblyResult {
    /// The assembled prose `## [Heading: name]\n<body>` block (LLM-facing prior
    /// knowledge). Empty for `ActionShortCircuit` / `Disambiguation`.
    pub orchestrator_content: String,
    /// UUIDs of matched components (the orchestrator-channel identity set, or
    /// the single override / action id).
    pub matched_component_ids: Vec<String>,
    /// `true` for the Solution-Override path (§3.13) — `orchestrator_content`
    /// is the verbatim override body and replaces the normal prompt assembly.
    pub override_prompt_creation: bool,
    /// `true` when an Action (class 16) intent match short-circuited (no LLM
    /// prior knowledge). Vestigial under Q2 (routes via `RetrievalTurnResult`).
    pub action_short_circuit: bool,
    /// The short-circuited Action component id. `None` unless
    /// `action_short_circuit`. Vestigial under Q2.
    pub action_component_id: Option<String>,
    /// The short-circuited Action name. `None` unless `action_short_circuit`.
    /// Vestigial under Q2.
    pub action_name: Option<String>,
    /// `true` when multiple near-equal intent candidates require a user
    /// disambiguation prompt (§3.12 Q11). Vestigial under Q2.
    pub disambiguation: bool,
    /// Disambiguation candidate descriptors (`component_id` / `class_code` /
    /// `class_label` / `score`). Empty unless `disambiguation`. Vestigial.
    pub candidates: Vec<serde_json::Value>,
    /// `true` when the matched Recipe declared a Tier-0 (no-LLM) channel.
    /// Vestigial under Q2 (routes via `RetrievalTurnResult.tier0_eligible`).
    pub tier_zero: bool,
}

/// Result of Tier-0 no-LLM orchestrator-channel execution
/// (`execute_tier_zero_channel`, H8.3).
///
/// `formatted_output` is the reply text extracted from the last successful
/// PythonCode step (`final_answer` → `return_value` stringified → `stdout` →
/// `""`, mirroring the Python `execute_recipe_orchestrator_channel` reference).
/// `matched_component_ids` carries the orchestrator-channel UUIDs for Wilson
/// scoring (`record_recipe_outcome`). A channel parse failure, a
/// non-PythonCode step, or any step failure (incl. an approval-gate pause)
/// degrades to an empty result so the caller falls back to Tier 2 — matching
/// the Python `{outcome:"error"}` → Tier-2 degradation.
#[derive(Debug, Clone, PartialEq)]
pub struct TierZeroChannelResult {
    /// The reply text to emit as the assistant reply (Tier-0, no LLM call).
    pub formatted_output: String,
    /// UUIDs of the orchestrator-channel components that produced this reply.
    pub matched_component_ids: Vec<String>,
}

/// Pure-Rust Tier-1 prior-knowledge assembly (v3 Phase H8.2). Replaces the
/// dormant Model A `__assemble_prior_knowledge__` Monty handler with a direct
/// library call for the composition `OrchestratorLookup::run_step_zero` bridge
/// (H.7 / H.12).
///
/// **`recipe_hint` shape = Option C** (user lock): `Some(v)` → `v` is a
/// serialized `Vec<ComponentItem>` stashed by `RecipeStage` (the
/// orchestrator-channel items from `RetrievalTurnResult`); the fn assembles
/// them with NO second `fetch_for_turn`. `None` → no stash, so the fn performs
/// a fresh `retrieval_source.fetch_for_turn(...)` and runs the full
/// `FetchForTurnResult` arm logic.
///
/// **Routing is NOT returned here** (user Q2 lock) — the Tier-0/Tier-1
/// decision for Model B/C comes from `RetrievalTurnResult` (H.4), branched on
/// inline in `RecipeStage::process` (Phase H.10); the matched
/// `orchestrator_items` / `rust_items` are stashed on
/// `LoopExecutionState.recipe_hint` / `recipe_rust_context` (H.9).
/// `RecipeStage` routes
/// action-short-circuit / disambiguation / Tier-0 cases away from
/// `run_step_zero` (Tier 1) before this fn is called, so the
/// `action_*` / `disambiguation` / `tier_zero` fields on the result are
/// vestigial on the Some-branch (defaulted false); the None-branch still
/// populates them faithfully from the `FetchForTurnResult` arms so the struct
/// stays a faithful projection of the retrieval outcome.
pub async fn assemble_prior_knowledge_with_hint(
    thread: &Thread,
    goal: &str,
    token_budget: usize,
    sender_class_code: &str,
    retrieval_source: Option<&Arc<dyn RetrievalSource>>,
    recipe_hint: Option<serde_json::Value>,
) -> Result<PkrAssemblyResult, EngineError> {
    // Some-branch: assemble the stashed orchestrator_items (Tier-1, no re-fetch).
    if let Some(hint) = recipe_hint {
        let items: Vec<crate::memory::ComponentItem> =
            serde_json::from_value(hint).map_err(|e| EngineError::InvalidInput {
                reason: format!(
                    "assemble_prior_knowledge_with_hint: recipe_hint deserialize failed: {e}"
                ),
            })?;
        return Ok(assemble_pkr_from_items(&items));
    }

    // None-branch: fresh fetch_for_turn + full arm logic.
    let Some(source) = retrieval_source else {
        return Ok(empty_pkr_assembly_result());
    };
    let scope = ComponentScope {
        tenant_id: thread.tenant_id.clone(),
        user_id: thread.user_id.clone(),
        agent_id: thread.agent_id.clone(),
        project_id: thread.project_id.to_string(),
    };
    match source
        .fetch_for_turn(&scope, goal, token_budget, sender_class_code)
        .await
    {
        Ok(fetch) => Ok(assemble_pkr_from_fetch(fetch)),
        Err(e) => {
            debug!("assemble_prior_knowledge_with_hint: fetch_for_turn failed: {e}");
            Ok(empty_pkr_assembly_result())
        }
    }
}

/// Assemble a [`PkrAssemblyResult`] from a slice of orchestrator-channel
/// [`ComponentItem`]s — the Some-branch of
/// [`assemble_prior_knowledge_with_hint`] AND the `Components` arm of
/// [`assemble_pkr_from_fetch`]. Shared so both branches emit identical
/// `orchestrator_content` for the same items.
///
/// Solution Override (§3.13): exactly one item with `override_prompt_creation`
/// → verbatim body + `override_prompt_creation = true`. Otherwise the normal
/// prose `## [Heading: name]\n<body>` block via [`format_orchestrator_content`]
/// and the full id list. Routing fields default to the non-short-circuit /
/// non-disambiguation / Tier-1 shape (the Tier-0/Tier-1 decision is upstream,
/// carried on `RetrievalTurnResult`).
pub fn assemble_pkr_from_items(items: &[crate::memory::ComponentItem]) -> PkrAssemblyResult {
    let override_items: Vec<_> = items
        .iter()
        .filter(|item| item.override_prompt_creation)
        .collect();
    if override_items.len() == 1 {
        let item = override_items[0];
        return PkrAssemblyResult {
            orchestrator_content: item.effective_content.clone(),
            matched_component_ids: vec![item.id.to_string()],
            override_prompt_creation: true,
            action_short_circuit: false,
            action_component_id: None,
            action_name: None,
            disambiguation: false,
            candidates: Vec::new(),
            tier_zero: false,
        };
    }
    PkrAssemblyResult {
        orchestrator_content: format_orchestrator_content(items),
        matched_component_ids: items.iter().map(|item| item.id.to_string()).collect(),
        override_prompt_creation: false,
        action_short_circuit: false,
        action_component_id: None,
        action_name: None,
        disambiguation: false,
        candidates: Vec::new(),
        tier_zero: false,
    }
}

/// Project a [`crate::memory::retrieval_source::FetchForTurnResult`] (fresh
/// `fetch_for_turn`) into a [`PkrAssemblyResult`] — the None-branch of
/// [`assemble_prior_knowledge_with_hint`]. Faithful to the retired Model A
/// `handle_assemble_prior_knowledge` arm logic (H8.4 deletes the handler; this
/// preserves the behaviour for the no-stash path and for direct unit-test
/// coverage of every `FetchForTurnResult` arm).
fn assemble_pkr_from_fetch(
    result: crate::memory::retrieval_source::FetchForTurnResult,
) -> PkrAssemblyResult {
    use crate::memory::retrieval_source::FetchForTurnResult;

    match result {
        FetchForTurnResult::Components(items) => assemble_pkr_from_items(&items),
        FetchForTurnResult::Disambiguation(candidates) => {
            let candidates_json: Vec<serde_json::Value> = candidates
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "component_id": c.component_id.to_string(),
                        "component_class_code": c.component_class_code,
                        "class_label": c.class_label,
                        "score": c.score,
                    })
                })
                .collect();
            PkrAssemblyResult {
                orchestrator_content: String::new(),
                matched_component_ids: Vec::new(),
                override_prompt_creation: false,
                action_short_circuit: false,
                action_component_id: None,
                action_name: None,
                disambiguation: true,
                candidates: candidates_json,
                tier_zero: false,
            }
        }
        FetchForTurnResult::ActionShortCircuit { component_id, name } => PkrAssemblyResult {
            orchestrator_content: String::new(),
            matched_component_ids: vec![component_id.to_string()],
            override_prompt_creation: false,
            action_short_circuit: true,
            action_component_id: Some(component_id.to_string()),
            action_name: Some(name),
            disambiguation: false,
            candidates: Vec::new(),
            tier_zero: false,
        },
        FetchForTurnResult::SplitResult {
            orchestrator_items,
            routing,
            ..
        } => PkrAssemblyResult {
            orchestrator_content: format_orchestrator_content(&orchestrator_items),
            matched_component_ids: routing.matched_component_ids,
            override_prompt_creation: routing.override_prompt_creation,
            action_short_circuit: false,
            action_component_id: None,
            action_name: None,
            disambiguation: false,
            candidates: Vec::new(),
            tier_zero: !routing.llm_call_required,
        },
    }
}

/// Empty / degrade-graceful [`PkrAssemblyResult`] — no prior knowledge, no
/// routing signals. Returned when there is no `retrieval_source` (None-branch)
/// or `fetch_for_turn` errors; the caller (composition
/// `OrchestratorLookup::run_step_zero`) surfaces this as a no-op bundle so the
/// Tier-1 turn proceeds without a prior-knowledge prepend (degrade to Tier 2).
fn empty_pkr_assembly_result() -> PkrAssemblyResult {
    PkrAssemblyResult {
        orchestrator_content: String::new(),
        matched_component_ids: Vec::new(),
        override_prompt_creation: false,
        action_short_circuit: false,
        action_component_id: None,
        action_name: None,
        disambiguation: false,
        candidates: Vec::new(),
        tier_zero: false,
    }
}

/// A parsed orchestrator-channel step (v3 Phase H8.3) — the Rust port of the
/// Python `_parse_orchestrator_channel_steps` (default.py) `{kind, name, body}`
/// dict. Produced by [`parse_orchestrator_channel_steps`] and consumed by
/// [`execute_tier_zero_channel`].
///
/// `pub` so the composition `OrchestratorLookup` bridge (H.12) and Tier-0 tests
/// can inspect a parsed channel without duplicating the private parse logic
/// (formatter-visibility decision, locked 2026-08-27).
#[derive(Debug, Clone, PartialEq)]
pub struct OrchestratorChannelStep {
    /// The `StepContextSpec` category label from the `## [Label: name]` heading
    /// (Skill / PythonCode / …). Only `PythonCode` steps are executable at
    /// Tier 0 (FIND-P9-02 Q1).
    pub kind: String,
    /// The component name from the heading.
    pub name: String,
    /// The `effective_content` body (`""` for a heading-only block).
    pub body: String,
}

/// Parse the orchestrator-channel prior-knowledge prose block format (v3 Phase
/// H8.3) — the Rust port of Python `_parse_orchestrator_channel_steps`
/// (default.py). [`format_orchestrator_content`] emits each `ComponentItem` as
/// a heading line `## [{Heading}: {name}]` optionally followed by its
/// `effective_content` body; consecutive blocks are separated by a blank line
/// (`\n\n`). Class 13 (ToolSkill) + class 11 are skipped by the formatter, so
/// they never appear here. A heading-only block (empty body) is just the
/// heading line with no body.
///
/// Returns `Ok(vec![])` for an empty input. Returns
/// [`EngineError::InvalidInput`] on a block whose first line is not a
/// `## [Label: name]` heading or is missing the `: ` separator;
/// [`execute_tier_zero_channel`] converts this to an empty
/// [`TierZeroChannelResult`] degrade (mirroring the Python `outcome:"error"`
/// → Tier-2 degradation).
pub fn parse_orchestrator_channel_steps(
    content: &str,
) -> Result<Vec<OrchestratorChannelStep>, EngineError> {
    if content.is_empty() {
        return Ok(Vec::new());
    }
    let mut steps = Vec::new();
    for block in content.split("\n\n") {
        if block.is_empty() {
            continue;
        }
        let mut lines = block.split('\n');
        // `block` is non-empty here (empty blocks were skipped), so
        // `block.split('\n')` always yields at least one element.
        let first = lines.next().unwrap_or("");
        if !first.starts_with("## [") || !first.ends_with(']') {
            return Err(EngineError::InvalidInput {
                reason: format!(
                    "orchestrator channel step missing '## [Label: name]' heading: {first}"
                ),
            });
        }
        // `first` starts with `## [` (ASCII, 4 bytes) and ends with `]`
        // (ASCII), so byte-slicing at `4` and `len-1` is char-boundary safe.
        let inner = &first[4..first.len() - 1];
        let Some((kind, name)) = inner.split_once(": ") else {
            return Err(EngineError::InvalidInput {
                reason: format!(
                    "orchestrator channel step heading missing ': ' separator: {first}"
                ),
            });
        };
        let body = lines.collect::<Vec<_>>().join("\n");
        steps.push(OrchestratorChannelStep {
            kind: kind.to_string(),
            name: name.to_string(),
            body,
        });
    }
    Ok(steps)
}

/// Run the Tier-0 orchestrator channel (PythonCode bodies + tool calls) with
/// NO LLM (v3 Phase H8.3 / plan §0.23 Gap3). The Rust embodiment of the Python
/// `execute_recipe_orchestrator_channel` (default.py) — both implement the
/// same logic, one in Python (Model A engine path), one in Rust (Model B/C
/// `LoopOrchestratorPort` bridge, H.12).
///
/// Flow (mirrors the Python reference):
/// 1. Parse `orchestrator_content` into steps via
///    [`parse_orchestrator_channel_steps`]. A parse failure (malformed heading
///    / missing `: ` separator) → empty [`TierZeroChannelResult`] degrade
///    (Python `outcome:"error"`).
/// 2. Only `kind == "PythonCode"` steps are executable at Tier 0 (FIND-P9-02
///    Q1: Tier-0 recipes are PythonCode-only; Skill bodies are LLM prose).
///    Any non-PythonCode step, or an empty step list, → empty degrade.
/// 3. Each PythonCode step runs via [`execute_code`] with a FRESH
///    [`ThreadExecutionContext`] per step and `persisted_state = {}`
///    (ISOLATION INVARIANT: no variables are shared between steps; each step
///    sees only the IBS-baked-in literals from `{{vars.slot0}}` substitution,
///    §0.20.3 — there is NO runtime `vars` dict). `capability_policies = &[]`.
/// 4. A step fails when [`execute_code`] returns `Err` OR its result has
///    `failure.is_some()` (internal error) OR `need_approval.is_some()` (a
///    tool call paused on an approval gate — per Q-H4 the channel signal is
///    binary success/error, so a gate pause degrades to empty; the Tier-2 LLM
///    path owns full gate handling, so the user's request still proceeds). On
///    first failure → empty degrade.
/// 5. All-success → reply text from the LAST step (Q-H4 / Q-H5result):
///    `final_answer` (from `FINAL("...")`) → else `return_value` stringified
///    → else captured `stdout` → else `""`.
///
/// **`matched_component_ids` is returned empty** — the locked Gap3 signature
/// (user-locked) carries no component-identity arg, and `orchestrator_content`
/// is the prose `## [Label: name]` block (component NAMES, not UUIDs); the
/// `rust_context` arg carries pre-loaded ToolSkill bindings (plan §5854), not
/// orchestrator-channel identity. The composition `OrchestratorLookup::
/// run_tier_zero` impl (H.12) supplies `TierZeroReply.matched_component_ids`
/// from the stashed `recipe_hint` (`Vec<ComponentItem>` with real UUIDs).
///
/// **`rust_context` is reserved** for the future ToolSkill-binding pre-load
/// (plan TIER0-GAP step 1: "applies the stashed `recipe_rust_context` to the
/// Rust execution context"). It is intentionally NOT consumed in the per-step
/// loop — the ISOLATION invariant requires fresh `{}` per step, and the
/// Python reference `__execute_code_step__(body, {})` passes no rust_context
/// either. The pre-load mechanism is wired when `recipe_rust_context` gains a
/// producer (H.9/H.10); until then the arg is accepted and unused.
///
/// Step events (`result.events`) are broadcast via `event_tx` (mirroring
/// `handle_execute_code_step`). `thread` is taken by shared reference (locked
/// signature) so events are NOT pushed to `thread.events` (unlike the Monty
/// handler) — the agent-loop owns its own state; the `event_tx` broadcast is
/// the trace/observer surface.
#[allow(clippy::too_many_arguments)]
pub async fn execute_tier_zero_channel(
    thread: &Thread,
    orchestrator_content: &str,
    _rust_context: &serde_json::Value,
    effects: &Arc<dyn EffectExecutor>,
    leases: &Arc<LeaseManager>,
    policy: &Arc<PolicyEngine>,
    gate_controller: &Arc<dyn crate::gate::GateController>,
    llm: &Arc<dyn LlmBackend>,
    event_tx: Option<&tokio::sync::broadcast::Sender<ThreadEvent>>,
) -> Result<TierZeroChannelResult, EngineError> {
    let steps = match parse_orchestrator_channel_steps(orchestrator_content) {
        Ok(s) => s,
        Err(e) => {
            debug!("execute_tier_zero_channel: parse failed: {e}");
            return Ok(empty_tier_zero_channel_result());
        }
    };
    if steps.is_empty() {
        debug!("execute_tier_zero_channel: no orchestrator channel steps to execute");
        return Ok(empty_tier_zero_channel_result());
    }

    let mut last_result: Option<crate::executor::scripting::CodeExecutionResult> = None;
    for step in &steps {
        if step.kind != "PythonCode" {
            debug!(
                "execute_tier_zero_channel: tier-0 channel step is not PythonCode: {}",
                step.kind
            );
            return Ok(empty_tier_zero_channel_result());
        }
        let exec_ctx =
            thread_execution_context(thread, StepId::new(), None, gate_controller.clone());
        let fresh_state = serde_json::json!({});
        match Box::pin(execute_code(
            &step.body,
            thread,
            Some(llm),
            effects,
            leases,
            policy,
            &exec_ctx,
            &[],
            &fresh_state,
        ))
        .await
        {
            Ok(result) => {
                // Broadcast in-execution events to the trace/observer channel
                // (mirror handle_execute_code_step). `thread` is immutable
                // here, so — unlike the Monty handler — events are NOT pushed
                // to `thread.events`; the agent-loop owns its own state.
                for event_kind in &result.events {
                    let event = ThreadEvent::new(thread.id, event_kind.clone());
                    if let Some(tx) = event_tx {
                        let _ = tx.send(event);
                    }
                }
                if result.failure.is_some() {
                    debug!(
                        "execute_tier_zero_channel: step '{}' failed; degrading to Tier 2",
                        step.name
                    );
                    return Ok(empty_tier_zero_channel_result());
                }
                if result.need_approval.is_some() {
                    debug!(
                        "execute_tier_zero_channel: step '{}' paused on approval gate; degrading to Tier 2",
                        step.name
                    );
                    return Ok(empty_tier_zero_channel_result());
                }
                last_result = Some(result);
            }
            Err(e) => {
                debug!(
                    "execute_tier_zero_channel: step '{}' raised: {e}; degrading to Tier 2",
                    step.name
                );
                return Ok(empty_tier_zero_channel_result());
            }
        }
    }

    let reply_text = last_result
        .as_ref()
        .map(extract_tier_zero_reply_text)
        .unwrap_or_default();
    Ok(TierZeroChannelResult {
        formatted_output: reply_text,
        matched_component_ids: Vec::new(),
    })
}

/// Extract the Tier-0 reply text from the last step's
/// [`crate::executor::scripting::CodeExecutionResult`] (Q-H4 / Q-H5result):
/// `final_answer` (from `FINAL("...")`) → else `return_value` stringified →
/// else captured `stdout` → else `""`. A JSON `null` return value (Python
/// `None`) is treated as "no return value" so `stdout` is considered next,
/// matching the Python `if rv is not None` branch.
fn extract_tier_zero_reply_text(
    result: &crate::executor::scripting::CodeExecutionResult,
) -> String {
    if let Some(ref ans) = result.final_answer {
        return ans.clone();
    }
    if !result.return_value.is_null() {
        if let Some(s) = result.return_value.as_str() {
            return s.to_string();
        }
        return result.return_value.to_string();
    }
    if !result.stdout.is_empty() {
        return result.stdout.clone();
    }
    String::new()
}

/// Empty / degrade-graceful [`TierZeroChannelResult`] — no reply text, no
/// matched ids. Returned on parse failure, empty step list, a non-PythonCode
/// step, or the first step failure / approval-gate pause; the composition
/// `OrchestratorLookup::run_tier_zero` (H.12) surfaces this as `None` so
/// `RecipeStage` falls back to Tier 2 (a Tier-0 failure degrades to a normal
/// LLM call so the user still gets a reply).
fn empty_tier_zero_channel_result() -> TierZeroChannelResult {
    TierZeroChannelResult {
        formatted_output: String::new(),
        matched_component_ids: Vec::new(),
    }
}

/// Handle `__check_budget__()`.
fn handle_check_budget(thread: &Thread) -> ExtFunctionResult {
    let tokens_remaining = thread
        .config
        .max_tokens_total
        .map(|max| max.saturating_sub(thread.total_tokens_used))
        .unwrap_or(u64::MAX);

    let time_remaining_ms = thread
        .config
        .max_duration
        .map(|dur| {
            let elapsed = chrono::Utc::now()
                .signed_duration_since(thread.created_at)
                .num_milliseconds()
                .max(0) as u64;
            dur.as_millis() as u64 - elapsed.min(dur.as_millis() as u64)
        })
        .unwrap_or(u64::MAX);

    let usd_remaining = thread
        .config
        .max_budget_usd
        .map(|max| (max - thread.total_cost_usd).max(0.0));

    let result = serde_json::json!({
        "tokens_remaining": tokens_remaining,
        "time_remaining_ms": time_remaining_ms,
        "usd_remaining": usd_remaining,
    });

    ExtFunctionResult::Return(json_to_monty(&result))
}

// ── Reduction rules cache ──────────────────────────────────
//
// The orchestrator Python calls `__get_reduction_rules__()` on every
// prompt assembly when the assembled message list is over budget. To
// keep that hot path off the DB, the resolved rules are cached
// per-(project_id, user_id) in a process-wide map. REST handlers that
// mutate the rules call `invalidate_reduction_rules_cache()` to flush
// stale entries; until then, the cache serves the same Vec without
// touching the DB.
//
// The cache key intentionally excludes the rule tag — only one tag
// ("reduction_rule") is supported today. Adding more tags later
// requires widening the key.
type ReductionRuleCacheKey = (crate::types::project::ProjectId, String);
type ReductionRuleCacheValue = Vec<serde_json::Value>;
type ReductionRuleSlot = Arc<StdMutex<Option<ReductionRuleCacheValue>>>;
type ReductionRuleCacheMap = HashMap<ReductionRuleCacheKey, ReductionRuleSlot>;

static REDUCTION_RULE_CACHE: LazyLock<StdMutex<ReductionRuleCacheMap>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

/// Invalidate cached reduction rules. Currently a process-wide flush:
/// every cached entry is dropped on the next access. Called by the
/// REST layer when reduction rules are added, removed, or replaced.
///
/// Returns the number of cache slots cleared (mostly useful for tests).
pub fn invalidate_reduction_rules_cache() -> usize {
    let cache = REDUCTION_RULE_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut cleared = 0;
    for slot in cache.values() {
        // Use the same poison-recovery pattern as the rest of this module:
        // if a thread panicked while holding the inner mutex we recover the
        // guard rather than silently skipping the slot (which would leave
        // stale rules cached forever).
        let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            *guard = None;
            cleared += 1;
        }
    }
    cleared
}

/// Apply cached or freshly-loaded reduction rules for the given project/
/// user. If a fresh DB load fails, returns an empty list (the
/// orchestrator skips reduction rather than aborting).
async fn load_reduction_rules(
    project_id: crate::types::project::ProjectId,
    user_id: &str,
    store: Option<&Arc<dyn Store>>,
) -> Vec<serde_json::Value> {
    let Some(store) = store else {
        return Vec::new();
    };
    let key: ReductionRuleCacheKey = (project_id, user_id.to_string());
    let slot = {
        let mut cache = REDUCTION_RULE_CACHE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cache
            .entry(key)
            .or_insert_with(|| Arc::new(StdMutex::new(None)))
            .clone()
    };
    {
        let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = guard.as_ref() {
            return cached.clone();
        }
    }
    let fresh = match store.list_memory_docs(project_id, user_id).await {
        Ok(docs) => {
            let mut out: Vec<serde_json::Value> = Vec::new();
            for doc in docs {
                if !doc.tags.iter().any(|t| t == "reduction_rule") {
                    continue;
                }
                let value = match serde_json::from_str::<serde_json::Value>(&doc.content) {
                    Ok(v) => v,
                    Err(e) => {
                        debug!("reduction rule parse failed: {e}");
                        continue;
                    }
                };
                if let Some(arr) = value.as_array() {
                    for entry in arr {
                        if entry.is_object() {
                            out.push(entry.clone());
                        }
                    }
                } else if value.is_object() {
                    out.push(value);
                }
            }
            out
        }
        Err(e) => {
            debug!("reduction rule load failed: {e}");
            // Cache the empty result so subsequent calls on the same
            // (project_id, user_id) pair do not hammer a flaky DB on
            // every over-budget turn. An explicit `invalidate_reduction_rules_cache`
            // call (e.g. after a DB recovery) will clear the slot.
            let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(Vec::new());
            return Vec::new();
        }
    };
    {
        let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(fresh.clone());
    }
    fresh
}

/// Handle `__log_budget_warning__(field, value, message)`.
///
/// Emits a `BudgetWarning` event. The token budget is the only soft
/// budget — time and cost budgets remain hard-stops. This function is
/// called by the orchestrator when the soft threshold is crossed, before
/// the reduction pipeline tries to shrink the prompt.
fn handle_log_budget_warning(
    args: &[MontyObject],
    kwargs: &[(MontyObject, MontyObject)],
    thread: &mut Thread,
    event_tx: Option<&tokio::sync::broadcast::Sender<ThreadEvent>>,
) -> ExtFunctionResult {
    let field = if let Some(s) = args.first().map(monty_to_string) {
        s
    } else {
        extract_string_kwarg(kwargs, "field").unwrap_or_default()
    };
    let value = args
        .get(1)
        .and_then(|v| match v {
            MontyObject::Int(i) => Some(*i),
            _ => None,
        })
        .or_else(|| extract_i64_kwarg(kwargs, "value"))
        .unwrap_or(0);
    let message = if args.len() >= 3 {
        monty_to_string(&args[2])
    } else {
        extract_string_kwarg(kwargs, "message").unwrap_or_default()
    };

    let kind = EventKind::BudgetWarning {
        field,
        value,
        message,
    };
    let event = ThreadEvent::new(thread.id, kind);
    if let Some(tx) = event_tx {
        let _ = tx.send(event.clone());
    }
    thread.events.push(event);
    thread.updated_at = chrono::Utc::now();

    ExtFunctionResult::Return(MontyObject::None)
}

/// Handle `__get_reduction_rules__()`.
///
/// Returns the cached or freshly-loaded reduction rules for the active
/// thread's project/user, filtered by the `reduction_rule` tag and
/// parsed as a JSON array of objects. Session-isolated via the
/// thread's `project_id`+`user_id`.
async fn handle_get_reduction_rules(
    thread: &Thread,
    store: Option<&Arc<dyn Store>>,
) -> ExtFunctionResult {
    let rules = load_reduction_rules(thread.project_id, &thread.user_id, store).await;
    ExtFunctionResult::Return(json_to_monty(&serde_json::json!(rules)))
}

/// Handle `__get_actions__()`.
async fn handle_get_actions(
    thread: &mut Thread,
    effects: &Arc<dyn EffectExecutor>,
    leases: &Arc<LeaseManager>,
    store: Option<&Arc<dyn Store>>,
) -> ExtFunctionResult {
    if let Err(e) =
        reconcile_dynamic_tool_lease(thread, effects, leases, store, &crate::LeasePlanner::new())
            .await
    {
        warn_on_lease_refresh_failure("get_actions", &e);
    }

    let active_leases = leases.active_for_thread(thread.id).await;
    // Read-only path: `available_actions` doesn't pause, so an inert
    // controller is correct. Plumbing the live one here would buy
    // nothing.
    let actions_context = thread_execution_context(
        thread,
        StepId::new(),
        None,
        crate::gate::CancellingGateController::arc(),
    );
    match effects
        .available_actions(&active_leases, &actions_context)
        .await
    {
        Ok(actions) => {
            let actions_json: Vec<serde_json::Value> = actions
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "name": a.name,
                        "description": a.description,
                        "params": a.parameters_schema,
                    })
                })
                .collect();
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!(actions_json)))
        }
        Err(e) => {
            debug!("get_actions failed: {e}");
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!([])))
        }
    }
}

/// Load all `DocType::Skill` MemoryDocs visible to `thread` from the legacy
/// in-memory `Store` fallback path (`host.skill_list` / `__list_skills__` when
/// the skills-db fast path is unavailable). The Python orchestrator handles
/// scoring, selection, and injection — this just provides data access.
///
/// ## Setup-marker exclusion (v2 parity with v1 selector)
///
/// Before returning the skill list, this function filters out any
/// skill whose `metadata.activation.setup_marker` is already present
/// as a MemoryDoc title in the current project. In v2, workspace
/// files are stored as MemoryDocs keyed by title, so "does the marker
/// file exist" maps to "is there a MemoryDoc with that title" — and
/// we already have the full doc list in scope for the skill filter,
/// so this costs zero extra store calls.
///
/// This is the v2 equivalent of the `satisfied_setup_markers`
/// argument threaded through `brassclaw_skills::prefilter_skills` on
/// the v1 path. Both paths implement the same rule: a one-time setup
/// skill whose marker file has been written has finished its job and
/// should not keep burning activation budget on every subsequent turn.
///
/// Extracted (C.6 slice 4c-prep) so the composition-side `ComponentPort` impl
/// can delegate its MemoryDoc fallback here without duplicating the rule.
pub async fn list_skills_from_store(
    store: &Arc<dyn Store>,
    thread: &Thread,
) -> Vec<serde_json::Value> {
    // User's docs in their project (all doc types — skill filtering happens
    // below in the `filter(|d| d.doc_type == Skill)` pass).
    let mut docs = match store
        .list_memory_docs(thread.project_id, &thread.user_id)
        .await
    {
        Ok(d) => d,
        Err(e) => {
            debug!("__list_skills__: failed to load user docs: {e}");
            vec![]
        }
    };

    // Admin/shared skills across ALL projects (fixes multi-tenant visibility —
    // shared skills live in the owner's project but must be visible to all users
    // regardless of which per-user project their thread runs in).
    match store.list_skills_global().await {
        Ok(shared) => docs.extend(shared),
        Err(e) => debug!("__list_skills__: failed to load global skills: {e}"),
    }

    docs.sort_by_key(|d| d.id.0);
    docs.dedup_by_key(|d| d.id);

    // Build the set of existing non-skill doc titles (== workspace paths
    // in v2) once, so setup-marker filtering below is O(1) per skill.
    // Exclude Skill docs so a marker like "github" doesn't collide with
    // the skill doc of the same name.
    let existing_titles: std::collections::HashSet<&str> = docs
        .iter()
        .filter(|d| d.doc_type != crate::types::memory::DocType::Skill)
        .map(|d| d.title.as_str())
        .collect();

    docs.iter()
        .filter(|d| d.doc_type == crate::types::memory::DocType::Skill)
        .filter(|d| {
            // Setup-marker exclusion. If the skill's activation
            // metadata declares a setup_marker and a MemoryDoc with
            // that title already exists, the skill's setup has been
            // completed and we skip it.
            let marker = d
                .metadata
                .get("activation")
                .and_then(|a| a.get("setup_marker"))
                .and_then(|m| m.as_str());
            match marker {
                Some(m) if existing_titles.contains(m) => {
                    debug!(
                        skill = %d.title,
                        marker = %m,
                        "__list_skills__: excluding setup skill — marker already present"
                    );
                    false
                }
                _ => true,
            }
        })
        .map(|d| {
            serde_json::json!({
                "doc_id": d.id.0.to_string(),
                "title": d.title,
                "content": d.content,
                "metadata": d.metadata,
            })
        })
        .collect()
}

/// Handle `__list_skills__()` / `host.skill_list()` (C.6 slice 4c-prep).
///
/// Thin-calls [`ComponentPort::list_skills`] (the composition-side impl runs the
/// skills-db fast path — sorted `reborn_skills` — with the MemoryDoc `Store`
/// fallback above). Returns a Python list of skill dicts; the Python orchestrator
/// handles scoring, selection, and injection. No bridge (`None` port, e.g.
/// non-skills-db config / unit-test path with no store) → an empty list.
async fn handle_list_skills(
    _args: &[MontyObject],
    thread: &Thread,
    component_port: Option<&Arc<dyn crate::executor::ComponentPort>>,
) -> ExtFunctionResult {
    let skills = match component_port {
        Some(port) => port.list_skills(thread).await.unwrap_or_default(),
        None => Vec::new(),
    };
    ExtFunctionResult::Return(json_to_monty(&serde_json::json!(skills)))
}

/// Handle `__record_skill_usage__(doc_id, success)`.
///
/// Records that a skill was used in this thread. Called by the Python
/// orchestrator after skill-assisted execution completes.
async fn handle_record_skill_usage(
    args: &[MontyObject],
    store: Option<&Arc<dyn Store>>,
) -> ExtFunctionResult {
    let Some(store) = store else {
        return ExtFunctionResult::Return(MontyObject::None);
    };

    let doc_id_str = args.first().map(monty_to_string).unwrap_or_default();
    let success = args
        .get(1)
        .map(|o| matches!(o, MontyObject::Bool(true)))
        .unwrap_or(false);

    let Ok(uuid) = uuid::Uuid::parse_str(&doc_id_str) else {
        debug!("__record_skill_usage__: invalid doc_id: {doc_id_str}");
        return ExtFunctionResult::Return(MontyObject::None);
    };

    let tracker = crate::memory::SkillTracker::new(Arc::clone(store));
    if let Err(e) = tracker
        .record_usage(crate::types::memory::DocId(uuid), success)
        .await
    {
        debug!("__record_skill_usage__: failed: {e}");
    }

    ExtFunctionResult::Return(MontyObject::None)
}

/// Handle `__regex_match__(pattern, text) -> bool`.
///
/// Compiles `pattern` with a bounded size limit and returns whether it
/// matches anywhere in `text`. Invalid regex or a size-limit violation
/// returns `False` silently. Used by the Python skill selector for regex
/// pattern scoring (Monty has no `re` module).
///
/// **Security: ReDoS safety.** This handler accepts arbitrary patterns from
/// the Python orchestrator (which itself receives them from skill manifests)
/// and runs them on user-supplied text. Safety relies on the `regex` crate's
/// linear-time matching guarantee (no backreferences, no lookaround) plus the
/// 64 KiB compiled-size cap and DFA-size cap below. If the `regex` crate is
/// ever swapped for `fancy-regex` (which supports backreferences and is NOT
/// linear-time), this becomes a real ReDoS vector. This is enforced by
/// convention and documentation only — see the top-of-crate comment in
/// `crates/brassclaw_engine/src/lib.rs`. (A `#[cfg(feature = "fancy-regex")]
/// compile_error!` tripwire was evaluated but conflicts with
/// `cargo clippy --all-features` which is the standard CI command.)
fn handle_regex_match(args: &[MontyObject]) -> ExtFunctionResult {
    let pattern = args.first().map(monty_to_string).unwrap_or_default();
    let text = args.get(1).map(monty_to_string).unwrap_or_default();
    if pattern.is_empty() {
        return ExtFunctionResult::Return(MontyObject::Bool(false));
    }
    // Cap compiled regex size to prevent ReDoS (matches the 64 KiB limit used
    // by `LoadedSkill::compile_patterns` in `brassclaw_skills`). Also cap the
    // lazy-DFA cache: the `regex` crate's DFA can grow beyond `size_limit`
    // during matching, so `dfa_size_limit` is a separate defensive cap on
    // memory allocation from a crafted pattern over untrusted skill manifests.
    const MAX_REGEX_SIZE: usize = 1 << 16;
    let matched = match regex::RegexBuilder::new(&pattern)
        .size_limit(MAX_REGEX_SIZE)
        .dfa_size_limit(MAX_REGEX_SIZE)
        .build()
    {
        Ok(re) => re.is_match(&text),
        Err(e) => {
            debug!("__regex_match__: invalid pattern '{pattern}': {e}");
            false
        }
    };
    ExtFunctionResult::Return(MontyObject::Bool(matched))
}

/// Handle `__validate_component__(title, content, doc_type, metadata)`.
///
/// Intercepts self-improvement `memory_write` calls for protected components
/// (orchestrator code at title `orchestrator:main`, prompt overlays at title
/// `prompt:codeact_preamble`) and creates an update-candidate MemoryDoc that
/// enters Q1 (validation_status = `pending`, queue_code = `q1_auto`) instead
/// of writing directly to the store.
///
/// Non-protected titles are forwarded to normal storage via the trusted-write
/// path, mirroring the original `memory_write` behaviour for routine docs.
///
/// Spec §3.5 / §3.6: all code/component changes must pass validation before
/// applying; the validator cannot be patched by the self-improvement mission.
async fn handle_validate_component(
    args: &[MontyObject],
    thread: &Thread,
    store: Option<&Arc<dyn Store>>,
) -> ExtFunctionResult {
    use crate::types::memory::{DocType, MemoryDoc};

    let title = args.first().map(monty_to_string).unwrap_or_default();
    let content = args.get(1).map(monty_to_string).unwrap_or_default();
    let doc_type_str = args
        .get(2)
        .map(monty_to_string)
        .unwrap_or_else(|| "note".into());
    let extra_meta = args.get(3).map(monty_to_json).unwrap_or_default();

    if title.is_empty() || content.is_empty() {
        debug!("__validate_component__: empty title or content — no-op");
        return ExtFunctionResult::Return(json_to_monty(
            &serde_json::json!({"queued": false, "reason": "empty payload"}),
        ));
    }

    // Map doc_type string to the DocType enum (lenient fallback to Note).
    let doc_type = match doc_type_str.to_ascii_lowercase().as_str() {
        "skill" => DocType::Skill,
        "recipe" => DocType::Recipe,
        "tool_skill" | "toolskill" => DocType::ToolSkill,
        "lesson" => DocType::Lesson,
        "spec" => DocType::Spec,
        "plan" => DocType::Plan,
        _ => DocType::Note,
    };

    // Is this a protected component that must go through validation?
    let is_protected = crate::executor::prompt::is_protected_component_title(&title);

    // Orchestrator (class 10) and Scaffold (class 50) components require an
    // LLM code-audit before Q2 manual validation (spec §3.5 / §3.5.1).
    // We flag the candidate with `llm_audit_required: true` and
    // `llm_audit_status: "pending"` so the WebUI validate route (Phase 6)
    // can enforce the gate: the "Validate" button is disabled until the audit
    // returns clean (Phase 3 / Step 6 §3.4). The actual LLM audit call is
    // performed by `crate::executor::code_audit::run_code_audit()` which is
    // wired into the WebUI PUT /components/{class_code}/{id}/validate handler.
    let needs_llm_audit = is_protected; // orchestrator:main and prompt:codeact_preamble are class 10/prompt

    let Some(store) = store else {
        debug!("__validate_component__: no store available — skipping write");
        return ExtFunctionResult::Return(json_to_monty(
            &serde_json::json!({"queued": false, "reason": "no_store"}),
        ));
    };

    // Build the update-candidate metadata.
    let mut meta = serde_json::Map::new();
    meta.insert("validation_status".into(), serde_json::json!("pending"));
    meta.insert("queue_code".into(), serde_json::json!("q1_auto"));
    if is_protected {
        meta.insert("is_update_candidate".into(), serde_json::json!(true));
        meta.insert("consumer_tags".into(), serde_json::json!(["05:validator"]));
    }
    if needs_llm_audit {
        // Gate flag: WebUI validate handler checks this before allowing Q2.
        meta.insert("llm_audit_required".into(), serde_json::json!(true));
        meta.insert("llm_audit_status".into(), serde_json::json!("pending"));
    }
    // Merge caller-supplied metadata (non-overriding for our validation fields).
    if let serde_json::Value::Object(extra_obj) = extra_meta {
        for (k, v) in extra_obj {
            meta.entry(k).or_insert(v);
        }
    }

    let mut candidate = MemoryDoc::new(
        thread.project_id,
        thread.user_id.clone(),
        doc_type,
        title.clone(),
        content,
    )
    .with_tags(vec!["update_candidate".into(), "05:validator".into()]);
    candidate.metadata = serde_json::Value::Object(meta);

    let candidate_id = format!("{}", candidate.id.0);
    let save_result =
        crate::runtime::with_trusted_internal_writes(store.save_memory_doc(&candidate)).await;

    match save_result {
        Ok(_) => {
            debug!(
                title = %title,
                candidate_id = %candidate_id,
                is_protected,
                "validate_component: update-candidate queued in Q1"
            );
            ExtFunctionResult::Return(json_to_monty(&serde_json::json!({
                "queued": true,
                "candidate_id": candidate_id,
                "validation_status": "pending",
                "queue_code": "q1_auto",
                "llm_audit_required": needs_llm_audit,
                "llm_audit_status": if needs_llm_audit { "pending" } else { "not_required" },
            })))
        }
        Err(e) => {
            debug!(
                title = %title,
                error = %e,
                "validate_component: store write failed"
            );
            ExtFunctionResult::Error(monty::MontyException::new(
                monty::ExcType::RuntimeError,
                Some(format!("__validate_component__ store write failed: {e}")),
            ))
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────

/// Build the context variables injected into the orchestrator Python.
fn build_orchestrator_inputs(
    thread: &Thread,
    persisted_state: &serde_json::Value,
) -> (Vec<String>, Vec<MontyObject>) {
    let names = vec![
        "context".into(),
        "goal".into(),
        "actions".into(),
        "state".into(),
        "config".into(),
    ];

    // Build orchestrator bootstrap context. Prefer the internal execution
    // transcript when present, otherwise fall back to the user-visible transcript.
    let bootstrap_messages = if thread.internal_messages.is_empty() {
        &thread.messages
    } else {
        &thread.internal_messages
    };
    let context: Vec<serde_json::Value> = bootstrap_messages
        .iter()
        .map(|m| {
            // Serialize action_calls through the Python interchange shape
            // (`{name, call_id, params}`) so the bootstrap context is
            // round-trip compatible with `python_json_to_action_calls`.
            // Using bare `m.action_calls` here produces the canonical Rust
            // serde format (`{action_name, id, parameters}`), which the
            // Python orchestrator passes back verbatim on the next
            // turn — and `python_json_to_action_calls`
            // then fails with "missing field `name`", orphaning every
            // subsequent tool result. This code path feeds action_calls
            // into the Python working transcript and must use the same
            // `{name, call_id, params}` shape the orchestrator expects.
            let calls_json = m
                .action_calls
                .as_ref()
                .map(|calls| serde_json::Value::Array(action_calls_to_python_json(calls)));
            serde_json::json!({
                "role": format!("{:?}", m.role),
                "content": m.content,
                "action_name": m.action_name,
                "action_call_id": m.action_call_id,
                "action_calls": calls_json,
            })
        })
        .collect();

    // Build config
    let config = serde_json::json!({
        "max_iterations": thread.config.max_iterations,
        "max_tool_intent_nudges": thread.config.max_tool_intent_nudges,
        "enable_tool_intent_nudge": thread.config.enable_tool_intent_nudge,
        "require_action_attempt": thread.config.require_action_attempt,
        "max_action_requirement_nudges": thread.config.max_action_requirement_nudges,
        "max_consecutive_errors": thread.config.max_consecutive_errors,
        "max_tokens_total": thread.config.max_tokens_total,
        "max_budget_usd": thread.config.max_budget_usd,
        "model_context_limit": thread.config.model_context_limit,
        "enable_compaction": thread.config.enable_compaction,
        "compaction_threshold": thread.config.compaction_threshold,
        "depth": thread.config.depth,
        "max_depth": thread.config.max_depth,
        "step_count": thread.step_count,
        // Phase 3: soft prompt-assembly budget. Zero means no reduction;
        // Python reads this as `prompt_budget` and gates the entire
        // `_reduce_prompt` pipeline on `prompt_budget > 0`.
        "prompt_budget_tokens": thread.config.prompt_budget_tokens,
    });

    let values = vec![
        json_to_monty(&serde_json::json!(context)),
        MontyObject::String(thread.goal.clone()),
        json_to_monty(&serde_json::json!([])), // actions loaded dynamically via __get_actions__
        json_to_monty(persisted_state),
        json_to_monty(&config),
    ];

    (names, values)
}

/// JSON shape used to interchange `ActionCall`s with the Python orchestrator.
///
/// This is the *single* place that defines the field naming convention used
/// across the Python boundary. It is intentionally separate from the
/// canonical `ActionCall` type because:
///
/// - `ActionCall` uses Rust-idiomatic field names (`id`, `action_name`,
///   `parameters`) and is also persisted into Step records and ThreadEvents.
///   Renaming its serde fields would invalidate every existing row.
/// - The Python orchestrator uses friendlier names (`call_id`, `name`,
///   `params`) that read naturally in CodeAct prompts and `default.py`.
///
/// Without this type, the round-trip is asymmetric: Rust → Python uses one
/// shape, Python → Rust used `serde_json::from_value::<Vec<ActionCall>>`
/// which silently fails (`.ok()` swallows the error) and produces `None`,
/// which means assistant messages came back without `action_calls`. The
/// downstream effect is that every tool result looks orphaned to
/// `sanitize_tool_messages` and gets rewritten as a user message — losing
/// the assistant ↔ tool_result linkage the LLM needs to reason about prior
/// tool calls.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PythonActionCall {
    name: String,
    call_id: String,
    params: serde_json::Value,
}

impl From<&ActionCall> for PythonActionCall {
    fn from(c: &ActionCall) -> Self {
        Self {
            name: c.action_name.clone(),
            call_id: c.id.clone(),
            params: c.parameters.clone(),
        }
    }
}

impl From<PythonActionCall> for ActionCall {
    fn from(p: PythonActionCall) -> Self {
        Self {
            id: p.call_id,
            action_name: p.name,
            parameters: p.params,
        }
    }
}

/// Serialize a slice of `ActionCall`s into the Python interchange shape.
///
/// On serialization failure (essentially unreachable for `String + String +
/// Value`, but still possible if the `serde_json::Value` parameters tree
/// contains a key whose stringification fails), the entry is **dropped**
/// from the output rather than replaced with `Value::Null`. The previous
/// `unwrap_or_else(|_| Value::Null)` corrupted the array — Python's
/// `default.py` accesses `c.get("name")` / `c.get("call_id")` /
/// `c.get("params")` on each entry, so a `null` would crash with a Python
/// `AttributeError` and lose the entire LLM step. `filter_map` produces a
/// shorter array, which Python's tool-result loop handles correctly because
/// it iterates `range(len(results))` against the shortened call list. The
/// warn log is preserved so operators have a breadcrumb if it ever fires.
fn action_calls_to_python_json(calls: &[ActionCall]) -> Vec<serde_json::Value> {
    calls
        .iter()
        .filter_map(|c| match serde_json::to_value(PythonActionCall::from(c)) {
            Ok(value) => Some(value),
            Err(e) => {
                warn!(
                    error = %e,
                    action_name = %c.action_name,
                    "Failed to serialize ActionCall for Python orchestrator — dropping entry"
                );
                None
            }
        })
        .collect()
}

/// Build a PII-safe summary of an `action_calls` JSON value for log output.
///
/// The action_calls payload contains tool parameters, which can carry user
/// PII (search queries, file names, email content, conversation text).
/// Dumping the full value into a `warn!` log would leak that PII to log
/// aggregation systems (Datadog, CloudWatch, Sentry) the moment the parser
/// fails — and the parser only fails when the Python ↔ Rust shape drifts,
/// which is exactly when an operator is most likely to be grepping logs.
///
/// We emit only the structural information operators actually need to
/// debug a shape drift: array length and the keys of the first entry. The
/// keys themselves are not user data — they're field names like
/// `name`/`call_id`/`params` that are static across all calls.
fn summarize_action_calls_for_log(value: &serde_json::Value) -> String {
    match value.as_array() {
        Some(arr) if arr.is_empty() => "empty array".to_string(),
        Some(arr) => {
            let first_keys = arr
                .first()
                .and_then(|v| v.as_object())
                .map(|obj| {
                    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
                    keys.sort_unstable();
                    keys.join(",")
                })
                .unwrap_or_else(|| "<not an object>".to_string());
            format!(
                "array of {} entries; first entry keys: [{}]",
                arr.len(),
                first_keys
            )
        }
        None => format!("non-array value of type {}", json_value_type_name(value)),
    }
}

/// Cheap type-name string for a `serde_json::Value`. Used by
/// `summarize_action_calls_for_log` to surface the wrong-shape case
/// (e.g. Python passed a string instead of an array) without leaking the
/// actual contents.
fn json_value_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Deserialize an `action_calls` JSON array (in Python interchange shape)
/// back into canonical `ActionCall`s.
///
/// Logs a warning on failure rather than swallowing silently. The whole
/// commit that introduced this helper exists to undo a `.ok()` swallow that
/// dropped action_calls without any signal — replacing it with another
/// `.ok()?` would re-introduce the same trap, just one layer deeper. If the
/// shape ever drifts again (Python orchestrator field rename, extra
/// required field, partial migration), the warning is the operator-visible
/// breadcrumb that explains why subsequent tool results suddenly look
/// orphaned to `sanitize_tool_messages`.
///
/// The warn log emits a structural summary (`summarize_action_calls_for_log`)
/// instead of the raw value because tool parameters can contain user PII.
fn python_json_to_action_calls(value: &serde_json::Value) -> Option<Vec<ActionCall>> {
    match serde_json::from_value::<Vec<PythonActionCall>>(value.clone()) {
        Ok(parsed) => Some(parsed.into_iter().map(ActionCall::from).collect()),
        Err(e) => {
            warn!(
                error = %e,
                shape = %summarize_action_calls_for_log(value),
                "Failed to parse action_calls from Python orchestrator — \
                 assistant message will lose tool_call linkage and downstream \
                 tool results will be rewritten as user messages"
            );
            None
        }
    }
}

fn json_to_thread_messages(value: &serde_json::Value) -> Option<Vec<ThreadMessage>> {
    let arr = value.as_array()?;
    let mut messages = Vec::with_capacity(arr.len());

    for item in arr {
        let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("User");
        let content = item
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        // Filter out null before calling the parser — `action_calls: null`
        // is Python's legitimate "this message has no tool calls" signal (text
        // response), not a parse error. Without this filter, the warn log in
        // python_json_to_action_calls fires on every text-only assistant
        // message with "invalid type: null, expected a sequence".
        let action_calls = item
            .get("action_calls")
            .filter(|v| !v.is_null())
            .and_then(python_json_to_action_calls);

        let message = match role {
            "System" | "system" => ThreadMessage::system(content),
            "Assistant" | "assistant" => {
                if let Some(calls) = action_calls {
                    ThreadMessage::assistant_with_actions(Some(content.to_string()), calls)
                } else {
                    ThreadMessage::assistant(content)
                }
            }
            "ActionResult" | "action_result" => ThreadMessage::action_result(
                item.get("action_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default(),
                item.get("action_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default(),
                content,
            ),
            _ => ThreadMessage::user(content),
        };
        messages.push(message);
    }

    Some(messages)
}

fn sync_runtime_state(thread: &mut Thread, state: Option<&serde_json::Value>) {
    let Some(state) = state else {
        return;
    };
    if let Some(messages) = state
        .get("working_messages")
        .and_then(json_to_thread_messages)
    {
        thread.internal_messages = messages;
        thread.updated_at = chrono::Utc::now();
    }
}

fn sync_visible_outcome(thread: &mut Thread, outcome: &ThreadOutcome) {
    if let ThreadOutcome::Completed {
        response: Some(response),
    } = outcome
    {
        let already_present = thread
            .messages
            .last()
            .map(|msg| {
                msg.role == crate::types::message::MessageRole::Assistant
                    && msg.content == *response
            })
            .unwrap_or(false);
        if !already_present {
            thread.add_message(ThreadMessage::assistant(response));
        }
    }
}

/// Parse the orchestrator's return value into a ThreadOutcome.
fn parse_outcome(result: &serde_json::Value) -> ThreadOutcome {
    let outcome = result
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or("completed");

    match outcome {
        "completed" => ThreadOutcome::Completed {
            response: result
                .get("response")
                .and_then(|v| v.as_str())
                .map(String::from),
        },
        "stopped" => ThreadOutcome::Stopped,
        "max_iterations" => ThreadOutcome::MaxIterations,
        "failed" => ThreadOutcome::Failed {
            error: result
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string(),
            debug_detail: None,
        },
        "gate_paused" => {
            let resume_kind_value = result
                .get("resume_kind")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let resume_kind = serde_json::from_value(resume_kind_value).unwrap_or(
                crate::gate::ResumeKind::Approval {
                    allow_always: false,
                },
            );
            ThreadOutcome::GatePaused {
                gate_name: result
                    .get("gate_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                action_name: result
                    .get("action_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                call_id: result
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                parameters: result
                    .get("parameters")
                    .cloned()
                    .unwrap_or(serde_json::json!({})),
                resume_kind,
                resume_output: result.get("resume_output").cloned(),
                paused_lease: result
                    .get("paused_lease")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok()),
            }
        }
        _ => ThreadOutcome::Completed { response: None },
    }
}

fn extract_string_arg(
    args: &[MontyObject],
    kwargs: &[(MontyObject, MontyObject)],
    name: &str,
    position: usize,
) -> Option<String> {
    for (k, v) in kwargs {
        if let MontyObject::String(key) = k
            && key == name
        {
            return Some(monty_to_string(v));
        }
    }
    args.get(position).map(monty_to_string)
}

fn extract_string_kwarg(kwargs: &[(MontyObject, MontyObject)], name: &str) -> Option<String> {
    for (k, v) in kwargs {
        if let MontyObject::String(key) = k
            && key == name
        {
            return Some(monty_to_string(v));
        }
    }
    None
}

fn extract_u64_kwarg(kwargs: &[(MontyObject, MontyObject)], name: &str) -> Option<u64> {
    for (k, v) in kwargs {
        if let MontyObject::String(key) = k
            && key == name
            && let MontyObject::Int(i) = v
        {
            return Some(*i as u64);
        }
    }
    None
}

fn extract_i64_kwarg(kwargs: &[(MontyObject, MontyObject)], name: &str) -> Option<i64> {
    for (k, v) in kwargs {
        if let MontyObject::String(key) = k
            && key == name
            && let MontyObject::Int(i) = v
        {
            return Some(*i);
        }
    }
    None
}

/// Build the JSON `args` payload for a dynamic cdylib Tool call from the Monty
/// call's positional args (excluding `self`) and kwargs. Positional args are
/// keyed `__arg{i}`; kwargs use their string keys (non-string keys are
/// stringified). The cdylib receives this as `CdylibRequest.args`.
fn dynamic_call_args_to_json(
    args: &[MontyObject],
    kwargs: &[(MontyObject, MontyObject)],
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (i, arg) in args.iter().enumerate() {
        map.insert(format!("__arg{i}"), monty_to_json(arg));
    }
    for (key, value) in kwargs {
        let key_str = match key {
            MontyObject::String(s) => s.clone(),
            other => monty_to_string(other),
        };
        map.insert(key_str, monty_to_json(value));
    }
    serde_json::Value::Object(map)
}

/// Dispatch a `host.<name>(...)` call that is not a built-in to a loaded
/// dynamic cdylib Tool via the [`DynamicToolPort`]. Returns `NotFound` when the
/// tool is not loaded (so Monty can resolve user-defined names), `Return` with
/// the cdylib's JSON result on success, or `Error` on invocation failure.
fn dispatch_dynamic_tool(
    port: &dyn DynamicToolPort,
    tool_name: &str,
    args: &[MontyObject],
    kwargs: &[(MontyObject, MontyObject)],
) -> ExtFunctionResult {
    if !port.is_loaded(tool_name) {
        return ExtFunctionResult::NotFound(tool_name.to_string());
    }
    let args_json = dynamic_call_args_to_json(args, kwargs);
    match port.invoke(tool_name, args_json) {
        Ok(value) => ExtFunctionResult::Return(json_to_monty(&value)),
        Err(err) => ExtFunctionResult::Error(monty::MontyException::new(
            monty::ExcType::RuntimeError,
            Some(format!("dynamic tool '{tool_name}' failed: {err}")),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::effect::ThreadExecutionContext;
    use crate::memory::intent_system::{IntentCandidate, IntentResolution};
    use crate::memory::{
        ComponentItem, ComponentScope, FetchForTurnResult, RetrievalEngine, RetrievalSource,
        RetrievalSourceError, TurnRoutingSignals,
    };
    use crate::types::memory::{DocType, MemoryDoc};
    use crate::types::project::ProjectId;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // ── C.3 slice 5: dynamic cdylib Tool dispatch fallthrough ───────────────
    use crate::executor::DynamicToolPortError;

    /// A mock [`DynamicToolPort`] for the dispatch-fallthrough unit tests.
    /// Records the last invoke args and returns a canned result.
    struct MockDynamicToolPort {
        loaded: Mutex<bool>,
        invoke_result: Mutex<Option<Result<serde_json::Value, DynamicToolPortError>>>,
        last_args: Mutex<Option<serde_json::Value>>,
    }

    impl MockDynamicToolPort {
        fn new(loaded: bool) -> Self {
            Self {
                loaded: Mutex::new(loaded),
                invoke_result: Mutex::new(None),
                last_args: Mutex::new(None),
            }
        }
    }

    impl super::DynamicToolPort for MockDynamicToolPort {
        fn is_loaded(&self, _tool_name: &str) -> bool {
            *self.loaded.lock().unwrap()
        }
        fn invoke(
            &self,
            tool_name: &str,
            args: serde_json::Value,
        ) -> Result<serde_json::Value, DynamicToolPortError> {
            *self.last_args.lock().unwrap() = Some(args.clone());
            self.invoke_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or(Ok(args))
                .map_err(|mut e| {
                    // keep the recorded tool name aligned with the call
                    if let DynamicToolPortError::Invoke { tool, .. } = &mut e {
                        *tool = tool_name.to_string();
                    }
                    e
                })
        }
    }

    #[test]
    fn dynamic_call_args_to_json_builds_kwargs_object() {
        let args = vec![monty::MontyObject::String("p".into())];
        let kwargs = vec![(
            monty::MontyObject::String("x".into()),
            monty::MontyObject::String("y".into()),
        )];
        let json = super::dynamic_call_args_to_json(&args, &kwargs);
        assert_eq!(json, serde_json::json!({"__arg0": "p", "x": "y"}));
    }

    #[test]
    fn dispatch_dynamic_tool_loaded_returns_json_as_monty() {
        let port = MockDynamicToolPort::new(true);
        *port.invoke_result.lock().unwrap() = Some(Ok(serde_json::json!({"echoed": true})));
        let kwargs = vec![(
            monty::MontyObject::String("k".into()),
            monty::MontyObject::String("v".into()),
        )];
        let result = super::dispatch_dynamic_tool(
            &port,
            "fixture_echo",
            &[monty::MontyObject::String("p".into())],
            &kwargs,
        );
        match result {
            monty::ExtFunctionResult::Return(obj) => {
                assert_eq!(
                    super::monty_to_json(&obj),
                    serde_json::json!({"echoed": true})
                );
            }
            other => panic!("expected Return, got {other:?}"),
        }
        // the dispatch forwarded kwargs+positional as the JSON args payload
        assert_eq!(
            port.last_args.lock().unwrap().clone(),
            Some(serde_json::json!({"__arg0": "p", "k": "v"}))
        );
    }

    #[test]
    fn dispatch_dynamic_tool_not_loaded_returns_not_found() {
        let port = MockDynamicToolPort::new(false);
        let result = super::dispatch_dynamic_tool(&port, "fixture_echo", &[], &[]);
        assert!(matches!(
            result,
            monty::ExtFunctionResult::NotFound(ref n) if n == "fixture_echo"
        ));
    }

    #[test]
    fn dispatch_dynamic_tool_invoke_error_returns_exception() {
        let port = MockDynamicToolPort::new(true);
        *port.invoke_result.lock().unwrap() = Some(Err(DynamicToolPortError::Invoke {
            tool: "fixture_echo".into(),
            reason: "boom".into(),
        }));
        let result = super::dispatch_dynamic_tool(&port, "fixture_echo", &[], &[]);
        assert!(matches!(result, monty::ExtFunctionResult::Error(_)));
    }

    // ── C.4.5.17 Part 3a: host.compose_orchestrator handler ────────────────
    use crate::executor::{ComponentPort, ComponentPortError};
    use crate::memory::composition::{ComposedProgram, ComposedStep, RustDirective, SkillRef};
    use std::future::Future;
    use std::pin::Pin;

    /// A mock [`ComponentPort`] for the compose_orchestrator handler tests.
    /// Returns a canned [`ComposedProgram`] (or an injected `Err`).
    struct MockComponentPort {
        result: Mutex<Option<Result<ComposedProgram, ComponentPortError>>>,
    }

    impl MockComponentPort {
        fn ok() -> Self {
            Self {
                result: Mutex::new(Some(Ok(ComposedProgram {
                    skills: vec![SkillRef {
                        id: uuid::Uuid::nil(),
                        class_code: 1,
                        name: "skill-fixture".into(),
                        body: "do the thing".into(),
                    }],
                    steplist: vec![ComposedStep {
                        step_id: "0:1".into(),
                        instructions: "run fixture".into(),
                        executable_code: "host.run_program('x')".into(),
                        tool_bindings: Vec::new(),
                    }],
                    rust_directives: vec![RustDirective {
                        tool_id: uuid::Uuid::nil(),
                        tool_name: "fixture_tool".into(),
                        artifact_path: "/tmp/fixture.cdylib".into(),
                    }],
                    variables: vec![("slot0".into(), "v0".into())],
                    assembled_program: "host.run_program('x')".into(),
                    tier: "tier0".into(),
                }))),
            }
        }
        fn failing(err: ComponentPortError) -> Self {
            Self {
                result: Mutex::new(Some(Err(err))),
            }
        }
    }

    impl ComponentPort for MockComponentPort {
        fn resolve_intent(
            &self,
            _scope: &ComponentScope,
            _user_input: &str,
        ) -> Pin<Box<dyn Future<Output = Result<IntentResolution, ComponentPortError>> + Send + '_>>
        {
            Box::pin(async { Err(ComponentPortError::Unavailable) })
        }

        fn fetch_component(
            &self,
            _scope: &ComponentScope,
            _component_id: uuid::Uuid,
            _class_code: i32,
        ) -> Pin<
            Box<dyn Future<Output = Result<Option<ComponentItem>, ComponentPortError>> + Send + '_>,
        > {
            Box::pin(async { Err(ComponentPortError::Unavailable) })
        }

        fn resolve_component_by_name(
            &self,
            _scope: &ComponentScope,
            _name: &str,
            _class_code: i32,
        ) -> Pin<
            Box<dyn Future<Output = Result<Option<ComponentItem>, ComponentPortError>> + Send + '_>,
        > {
            Box::pin(async { Err(ComponentPortError::Unavailable) })
        }

        fn list_skills(
            &self,
            _thread: &Thread,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Vec<serde_json::Value>, ComponentPortError>> + Send + '_,
            >,
        > {
            Box::pin(async { Err(ComponentPortError::Unavailable) })
        }

        fn compose(
            &self,
            _scope: &ComponentScope,
            _component_id: uuid::Uuid,
            _step_link: &str,
            _user_input: &str,
        ) -> Pin<Box<dyn Future<Output = Result<ComposedProgram, ComponentPortError>> + Send + '_>>
        {
            let result = self.result.lock().unwrap().clone();
            Box::pin(async move { result.expect("mock result must be injected") })
        }
    }

    #[tokio::test]
    async fn compose_orchestrator_no_port_returns_unavailable() {
        let thread = make_validate_thread();
        let args = vec![
            MontyObject::String(uuid::Uuid::nil().to_string()),
            MontyObject::String("0:1".into()),
            MontyObject::String("hi".into()),
        ];
        let result = handle_compose_orchestrator(&args, &thread, None).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(false));
        assert_eq!(json["error"], serde_json::json!("composition_unavailable"));
    }

    #[tokio::test]
    async fn compose_orchestrator_invalid_component_id_returns_error() {
        let thread = make_validate_thread();
        let port: Arc<dyn ComponentPort> = Arc::new(MockComponentPort::ok());
        let args = vec![
            MontyObject::String("not-a-uuid".into()),
            MontyObject::String("0:1".into()),
        ];
        let result = handle_compose_orchestrator(&args, &thread, Some(&port)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(false));
        assert_eq!(
            json["error"],
            serde_json::json!("missing or invalid component_id")
        );
    }

    #[tokio::test]
    async fn compose_orchestrator_missing_step_link_returns_error() {
        let thread = make_validate_thread();
        let port: Arc<dyn ComponentPort> = Arc::new(MockComponentPort::ok());
        let args = vec![MontyObject::String(uuid::Uuid::nil().to_string())];
        let result = handle_compose_orchestrator(&args, &thread, Some(&port)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(false));
        assert_eq!(json["error"], serde_json::json!("missing step_link"));
    }

    #[tokio::test]
    async fn compose_orchestrator_mock_port_returns_program() {
        let thread = make_validate_thread();
        let port: Arc<dyn ComponentPort> = Arc::new(MockComponentPort::ok());
        let args = vec![
            MontyObject::String(uuid::Uuid::nil().to_string()),
            MontyObject::String("0:1".into()),
            MontyObject::String("user text".into()),
        ];
        let result = handle_compose_orchestrator(&args, &thread, Some(&port)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(true));
        assert_eq!(json["program"]["tier"], serde_json::json!("tier0"));
        assert_eq!(
            json["program"]["steplist"][0]["step_id"],
            serde_json::json!("0:1")
        );
        assert_eq!(
            json["program"]["skills"][0]["name"],
            serde_json::json!("skill-fixture")
        );
        assert_eq!(
            json["program"]["rust_directives"][0]["tool_name"],
            serde_json::json!("fixture_tool")
        );
    }

    #[tokio::test]
    async fn compose_orchestrator_port_failure_surfaces_error() {
        let thread = make_validate_thread();
        let port: Arc<dyn ComponentPort> = Arc::new(MockComponentPort::failing(
            ComponentPortError::RecipeNotFound {
                component_id: uuid::Uuid::nil().to_string(),
            },
        ));
        let args = vec![
            MontyObject::String(uuid::Uuid::nil().to_string()),
            MontyObject::String("0:1".into()),
        ];
        let result = handle_compose_orchestrator(&args, &thread, Some(&port)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(false));
        assert!(json["error"].as_str().unwrap().contains("not found"));
    }

    // ── C.5: host.kohai_complete handler ───────────────────────────────────
    use crate::executor::kohai_port::{
        KohaiAnswer, KohaiCallCtx, KohaiPort, KohaiPortError, KohaiUsage,
    };

    /// A mock [`KohaiPort`] for the kohai_complete handler tests. Returns a
    /// canned [`KohaiAnswer`] (or an injected `Err`); the handler under test
    /// does not drive the provider call — the mock owns the result.
    struct MockKohaiPort {
        result: Mutex<Option<Result<KohaiAnswer, KohaiPortError>>>,
    }

    impl MockKohaiPort {
        fn ok() -> Self {
            Self {
                result: Mutex::new(Some(Ok(KohaiAnswer {
                    content: "kohai-answer".into(),
                    usage: KohaiUsage {
                        input_tokens: 11,
                        output_tokens: 22,
                        cost_usd: 0.003,
                    },
                }))),
            }
        }
        fn failing(err: KohaiPortError) -> Self {
            Self {
                result: Mutex::new(Some(Err(err))),
            }
        }
    }

    impl KohaiPort for MockKohaiPort {
        fn complete(
            &self,
            _prompt: serde_json::Value,
            _ctx: KohaiCallCtx,
        ) -> Pin<Box<dyn Future<Output = Result<KohaiAnswer, KohaiPortError>> + Send + 'static>>
        {
            let result = self.result.lock().unwrap().clone();
            Box::pin(async move { result.expect("mock result must be injected") })
        }
    }

    #[tokio::test]
    async fn kohai_complete_no_port_returns_unavailable() {
        let thread = make_validate_thread();
        let args = vec![json_to_monty(&serde_json::json!({
            "user_query": "hi",
            "chat_history": [],
            "prefix_placeholder": "{{prefix}}",
        }))];
        let result = handle_kohai_complete(&args, &[], &thread, None).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(false));
        assert_eq!(json["error"], serde_json::json!("kohai_unavailable"));
    }

    #[tokio::test]
    async fn kohai_complete_missing_prompt_returns_error() {
        let thread = make_validate_thread();
        let port: Arc<dyn KohaiPort> = Arc::new(MockKohaiPort::ok());
        let result = handle_kohai_complete(&[], &[], &thread, Some(&port)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(false));
        assert_eq!(
            json["error"],
            serde_json::json!("invalid prompt: missing or not a dict")
        );
    }

    #[tokio::test]
    async fn kohai_complete_non_dict_prompt_returns_error() {
        let thread = make_validate_thread();
        let port: Arc<dyn KohaiPort> = Arc::new(MockKohaiPort::ok());
        let args = vec![MontyObject::String("not-a-dict".into())];
        let result = handle_kohai_complete(&args, &[], &thread, Some(&port)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(false));
        assert_eq!(
            json["error"],
            serde_json::json!("invalid prompt: missing or not a dict")
        );
    }

    #[tokio::test]
    async fn kohai_complete_mock_port_returns_answer() {
        let thread = make_validate_thread();
        let port: Arc<dyn KohaiPort> = Arc::new(MockKohaiPort::ok());
        let prompt = serde_json::json!({
            "user_query": "hi",
            "chat_history": [],
            "prefix_placeholder": "{{prefix}}",
        });
        let kwargs = vec![(MontyObject::String("prompt".into()), json_to_monty(&prompt))];
        let result = handle_kohai_complete(&[], &kwargs, &thread, Some(&port)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(true));
        assert_eq!(json["answer"], serde_json::json!("kohai-answer"));
        assert_eq!(json["usage"]["input_tokens"], serde_json::json!(11));
        assert_eq!(json["usage"]["output_tokens"], serde_json::json!(22));
    }

    #[tokio::test]
    async fn kohai_complete_port_failure_surfaces_error() {
        let thread = make_validate_thread();
        let port: Arc<dyn KohaiPort> =
            Arc::new(MockKohaiPort::failing(KohaiPortError::LlmFailed {
                reason: "provider 502".into(),
            }));
        let args = vec![json_to_monty(&serde_json::json!({"user_query": "hi"}))];
        let result = handle_kohai_complete(&args, &[], &thread, Some(&port)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["ok"], serde_json::json!(false));
        assert!(json["error"].as_str().unwrap().contains("llm call failed"));
    }

    // ── Test constants ──────────────────────────────────────────────────────
    /// Max VM allocations for test helper runs (lower than production).
    const TEST_MAX_ALLOCATIONS: usize = 500_000;
    /// Max consecutive errors used in None-guard regression test.
    const TEST_CONSECUTIVE_ERRORS: i64 = 99;
    /// Negative token budget value used in event map tests.
    const TEST_NEG_TOKENS_50: i64 = -50;
    /// Budget token value in PromptOverBudget event shape tests.
    const TEST_BUDGET_TOKENS_6K: i64 = 6_000;
    /// Estimated token value in PromptOverBudget event shape tests.
    const TEST_ESTIMATED_TOKENS_8K: i64 = 8_000;
    /// Negative token value used in low-budget warning tests.
    const TEST_NEG_TOKENS_42: i64 = -42;
    /// Token allocation value used in emit-event tests.
    const TEST_TOKEN_ALLOC_2K: i64 = 2_000;

    // ── Orchestrator budget / error mapping ─────────────────────

    #[test]
    fn failure_reason_maps_timeout_to_user_safe_message() {
        let failure = classify_orchestrator_failure(
            "Orchestrator error after resume",
            "ResourceLimits: duration limit exceeded",
        );
        assert!(
            matches!(failure.kind, OrchestratorFailureKind::TimeLimit { .. }),
            "expected TimeLimit variant, got: {:?}",
            failure.kind
        );
        let rendered = failure.user_message();
        assert!(
            rendered.contains("time budget exhausted"),
            "expected user-safe timeout reason, got: {rendered}"
        );
        assert!(
            rendered.contains("DB-less mode")
                && rendered.contains("BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS"),
            "reason must mention DB-less-mode env var fallback, got: {rendered}"
        );
    }

    /// Regression for serrrfirat review on PR #2753 (commit 82d06410) —
    /// the classifier used to treat any `"timeout"` / `"timed out"`
    /// substring as a wall-clock exhaustion, so upstream LLM / network
    /// timeouts (`"Request timed out"`, `"Connection timed out"`) were
    /// mapped to `TimeLimit` and the user-facing message advised raising
    /// `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` — completely wrong for a
    /// provider-side timeout. Those now fall through to `Other` so the
    /// budget knob is only suggested when the failure is actually a
    /// Monty wall-clock limit.
    #[test]
    fn failure_reason_does_not_treat_upstream_timeout_as_time_limit() {
        for upstream in [
            "Request timed out",
            "Connection timed out",
            "LLM call failed: timeout waiting for response",
            "upstream provider timeout after 30s",
        ] {
            let failure = classify_orchestrator_failure("Orchestrator runtime error", upstream);
            assert!(
                !matches!(failure.kind, OrchestratorFailureKind::TimeLimit { .. }),
                "upstream timeout {upstream:?} must NOT classify as TimeLimit, got: {:?}",
                failure.kind,
            );
            assert!(
                matches!(failure.kind, OrchestratorFailureKind::Other { .. }),
                "upstream timeout {upstream:?} should fall through to Other, got: {:?}",
                failure.kind,
            );
            let rendered = failure.user_message();
            assert!(
                !rendered.contains("BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS"),
                "user message for upstream timeout must not advise raising the budget knob, got: {rendered}",
            );
        }
    }

    #[test]
    fn failure_reason_maps_memory_limit() {
        let failure =
            classify_orchestrator_failure("Orchestrator runtime error", "memory limit hit");
        assert!(matches!(
            failure.kind,
            OrchestratorFailureKind::ResourceLimit { .. }
        ));
        assert!(
            failure.user_message().contains("resource budget exhausted"),
            "memory-limit reason should not leak raw Monty text, got: {}",
            failure.user_message()
        );
    }

    /// Regression for Copilot review on PR #2753 (commit 042c2ee7) —
    /// `Other`'s user-facing Display used to embed the raw `err_msg`.
    /// Surfaces that render `format!("{error}")` directly bypass the
    /// channel-edge sanitizer in `bridge::user_facing_errors`, so any
    /// unclassified Monty output would leak tracebacks / internal paths
    /// there. The generic user-facing text now reads "internal orchestrator
    /// failure"; the raw message is preserved in `debug_detail`.
    #[test]
    fn failure_reason_hides_unknown_raw_message_from_user_text() {
        let failure =
            classify_orchestrator_failure("Orchestrator runtime error", "NameError: foo undefined");
        assert!(matches!(
            failure.kind,
            OrchestratorFailureKind::Other { .. }
        ));
        let rendered = failure.user_message();
        assert!(
            !rendered.contains("NameError"),
            "Other variant must not surface raw err_msg in Display, got: {rendered}"
        );
        assert!(
            rendered.contains("internal orchestrator failure"),
            "Other variant should render the generic fallback, got: {rendered}"
        );
        // Raw detail is still available for operator triage via debug_detail.
        assert!(
            failure.debug_detail().contains("NameError"),
            "debug_detail must preserve the raw err_msg, got: {}",
            failure.debug_detail()
        );
    }

    /// Regression for Copilot review on PR #2753 — substring `"duration"`
    /// alone mis-classified any error whose message happened to contain
    /// that word as a timeout. The narrow predicate set now requires
    /// the full phrase `"duration limit"` / `"max_duration"` /
    /// `"maximum duration"` or an explicit timeout word.
    #[test]
    fn failure_reason_does_not_treat_bare_duration_as_timeout() {
        let failure = classify_orchestrator_failure(
            "Orchestrator runtime error",
            "TypeError: duration must be a positive integer",
        );
        assert!(
            !matches!(failure.kind, OrchestratorFailureKind::TimeLimit { .. }),
            "bare 'duration' in an unrelated error must not classify as TimeLimit, got: {:?}",
            failure.kind
        );
        assert!(
            matches!(failure.kind, OrchestratorFailureKind::Other { .. }),
            "expected Other variant for unrelated duration-word error, got: {:?}",
            failure.kind
        );
    }

    #[test]
    fn failure_reason_strips_python_traceback() {
        // Regression for #2546 — bug bash 4/16 reported raw Python tracebacks
        // from the Monty VM being shown verbatim to end users, including
        // internal file paths ("orchestrator.py", line 907) and upstream
        // HTTP response bodies.
        let raw = "Traceback (most recent call last):\n  File \"orchestrator.py\", line 907, in run_loop\n  File \"orchestrator.py\", line 548, in __llm_complete__\nRuntimeError: LLM call failed: Provider nearai_chat request failed: HTTP 502 Bad Gateway";
        let failure = classify_orchestrator_failure("Orchestrator error after resume", raw);
        assert!(matches!(
            failure.kind,
            OrchestratorFailureKind::Traceback { .. }
        ));
        let rendered = failure.user_message();
        assert!(
            rendered.contains("internal orchestrator failure"),
            "should surface a generic internal-failure message, got: {rendered}"
        );
        for forbidden in [
            "Traceback",
            "orchestrator.py",
            "File \"",
            "line 907",
            "line 548",
            "HTTP 502",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "user-visible reason must not leak `{forbidden}`, got: {rendered}"
            );
        }
        // The debug detail MUST retain the raw traceback so gateway
        // debug mode can surface it without re-reading logs.
        assert!(
            failure.debug_detail().contains("Traceback"),
            "debug detail must preserve the raw Monty trace, got: {}",
            failure.debug_detail()
        );
    }

    #[test]
    fn max_duration_default_and_bounds() {
        // Default (no env var set): 300s — but OnceLock may already be
        // primed by another test in the suite, so we only check it's within
        // the documented bounds.
        let secs = orchestrator_max_duration().as_secs();
        assert!(
            (ORCHESTRATOR_MIN_MAX_DURATION_SECS..=ORCHESTRATOR_MAX_MAX_DURATION_SECS)
                .contains(&secs),
            "orchestrator_max_duration must be within [{ORCHESTRATOR_MIN_MAX_DURATION_SECS}, {ORCHESTRATOR_MAX_MAX_DURATION_SECS}], got {secs}"
        );
    }

    // ── C.6 slice 1: MontySession park/resume primitive ──────────────────────
    //
    // Drives a live Monty session through `host.await_next_turn()` + FINAL to
    // validate (a) the refactor that extracted execute_orchestrator's loop into
    // MontySession::drive_to_yield still completes a plain FINAL script, and
    // (b) the new `host.await_next_turn()` arm parks the VM and a second drive
    // resumes it to completion. The park/resume + FINAL path never touches the
    // LLM/effects/etc., so the mocks only need to exist.

    #[allow(clippy::type_complexity)]
    fn session_host_deps() -> (
        Arc<dyn EffectExecutor>,
        Arc<LeaseManager>,
        Arc<PolicyEngine>,
        Arc<dyn crate::gate::GateController>,
    ) {
        (
            Arc::new(NoopEffects),
            Arc::new(LeaseManager::new()),
            Arc::new(PolicyEngine::new()),
            Arc::new(crate::gate::CancellingGateController),
        )
    }

    fn session_fresh_thread() -> Thread {
        let mut thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            ProjectId::new(),
            "test-user",
            crate::types::thread::ThreadConfig::default(),
        );
        thread.transition_to(ThreadState::Running, None).unwrap();
        thread
    }

    #[tokio::test]
    async fn monty_session_drives_final_only_to_complete() {
        let (effects, leases, policy, gate) = session_host_deps();
        let mut thread = session_fresh_thread();
        let (_tx, mut signal_rx) = tokio::sync::mpsc::channel::<ThreadSignal>(8);
        let state = serde_json::json!({});
        let script = r#"FINAL({"outcome":"completed","response":"done"})"#;
        let mut session = MontySession::new(script, &thread, &state, None).unwrap();
        let yielded = session
            .drive_to_yield(
                &mut thread,
                &effects,
                &leases,
                &policy,
                &mut signal_rx,
                None,
                None,
                None,
                None,
                &gate,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert!(
            matches!(yielded, OrchestratorYield::Complete(_)),
            "plain FINAL script must complete"
        );
    }

    #[tokio::test]
    async fn monty_session_parks_on_await_next_turn_then_resumes() {
        let (effects, leases, policy, gate) = session_host_deps();
        let mut thread = session_fresh_thread();
        let (_tx, mut signal_rx) = tokio::sync::mpsc::channel::<ThreadSignal>(8);
        let state = serde_json::json!({});
        let script =
            "host.await_next_turn()\nFINAL({\"outcome\":\"completed\",\"response\":\"done\"})";
        let mut session = MontySession::new(script, &thread, &state, None).unwrap();

        // First drive: the script calls host.await_next_turn() → park.
        let first = session
            .drive_to_yield(
                &mut thread,
                &effects,
                &leases,
                &policy,
                &mut signal_rx,
                None,
                None,
                None,
                None,
                &gate,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert!(
            matches!(first, OrchestratorYield::AwaitNextTurn),
            "host.await_next_turn() must park the session"
        );

        // Second drive: resume the parked call with the next turn's input,
        // then run FINAL to completion.
        let second = session
            .drive_to_yield(
                &mut thread,
                &effects,
                &leases,
                &policy,
                &mut signal_rx,
                None,
                None,
                None,
                None,
                &gate,
                None,
                None,
                None,
                None,
                Some(json_to_monty(&serde_json::json!("next-turn-input"))),
            )
            .await
            .unwrap();
        assert!(
            matches!(second, OrchestratorYield::Complete(_)),
            "resumed session must complete"
        );
    }

    #[tokio::test]
    async fn prepare_monty_session_constructs_from_fresh_thread() {
        // A brand-new thread (no runtime checkpoint metadata, not yet
        // transitioned to Running) must yield a session: prepare_monty_session
        // loads the compiled-in DEFAULT_ORCHESTRATOR and an empty
        // persisted_state without requiring any Model-A bootstrap step.
        let thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            ProjectId::new(),
            "test-user",
            crate::types::thread::ThreadConfig::default(),
        );
        let session = prepare_monty_session(&thread, None, None).await;
        assert!(
            session.is_ok(),
            "prepare_monty_session must construct a session from a fresh thread"
        );
    }

    #[tokio::test]
    async fn prepare_monty_session_loads_real_orchestrator_and_parks() {
        // End-to-end: prepare_monty_session loads the real basic_mode.py
        // (DEFAULT_ORCHESTRATOR), builds the session, and the first drive parks
        // at host.await_next_turn() — the same gate as
        // default_orchestrator_parses_and_parks_at_first_await_next_turn, but
        // driven through the prepare helper the composition driver will use.
        let (effects, leases, policy, gate) = session_host_deps();
        let mut thread = session_fresh_thread();
        let (_tx, mut signal_rx) = tokio::sync::mpsc::channel::<ThreadSignal>(8);
        let mut session = prepare_monty_session(&thread, None, None)
            .await
            .expect("prepare must construct a session");
        let yielded = session
            .drive_to_yield(
                &mut thread,
                &effects,
                &leases,
                &policy,
                &mut signal_rx,
                None,
                None,
                None,
                None,
                &gate,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("first drive must not error");
        assert!(
            matches!(yielded, OrchestratorYield::AwaitNextTurn),
            "prepared real DEFAULT_ORCHESTRATOR must park at first await_next_turn"
        );
    }

    #[tokio::test]
    async fn default_orchestrator_parses_and_parks_at_first_await_next_turn() {
        // C.6 slice 2: the reworked basic_mode.py is a `while True` loop that
        // parks at host.await_next_turn(). No other test drives the real
        // DEFAULT_ORCHESTRATOR end-to-end, so this is the gate that verifies
        // the whole script (helpers + `while True` + host.check_signals +
        // host.await_next_turn) parses in Monty 0.0.16 and the first drive
        // parks at the await_next_turn (after the leading check_signals
        // returns None on an empty signal_rx).
        let (effects, leases, policy, gate) = session_host_deps();
        let mut thread = session_fresh_thread();
        let (_tx, mut signal_rx) = tokio::sync::mpsc::channel::<ThreadSignal>(8);
        let state = serde_json::json!({});
        let mut session = MontySession::new(DEFAULT_ORCHESTRATOR, &thread, &state, None)
            .expect("basic_mode.py must parse + start in Monty 0.0.16");
        let yielded = session
            .drive_to_yield(
                &mut thread,
                &effects,
                &leases,
                &policy,
                &mut signal_rx,
                None,
                None,
                None,
                None,
                &gate,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert!(
            matches!(yielded, OrchestratorYield::AwaitNextTurn),
            "basic_mode.py must park at the first host.await_next_turn()"
        );
    }

    // ── Python helper unit tests via Monty ──────────────────────
    //
    // Extracts the helper functions from the default orchestrator and
    // evaluates `signals_tool_intent(text)` directly, mirroring the V1
    // Rust unit test suite in crates/brassclaw_llm/src/reasoning.rs.

    /// Run a Python expression that returns a bool by prepending the
    /// orchestrator helper definitions and wrapping in `FINAL(expr)`.
    /// Run a Python snippet and drive the Monty VM, returning the FINAL()
    /// value as a `MontyObject`. This is the common core for `eval_python_bool`
    /// and `eval_python_int`.
    fn run_python_final(code: String) -> MontyObject {
        let runner =
            MontyRun::new(code, "test.py", vec![]).expect("Failed to parse orchestrator helpers");
        let mut stdout = String::new();
        let tracker =
            LimitedTracker::new(ResourceLimits::new().max_allocations(TEST_MAX_ALLOCATIONS));

        let mut progress = runner
            .start(vec![], tracker, PrintWriter::CollectString(&mut stdout))
            .expect("Failed to start orchestrator test");

        loop {
            match progress {
                RunProgress::Complete(obj) => return obj,
                RunProgress::FunctionCall(call) => {
                    if call.function_name == "FINAL" {
                        let val = call.args.first().cloned().unwrap_or(MontyObject::None);
                        let _ = call.resume(
                            ExtFunctionResult::Return(MontyObject::None),
                            PrintWriter::CollectString(&mut stdout),
                        );
                        return val;
                    }
                    let ext_result = match call.function_name.as_str() {
                        "__regex_match__" => handle_regex_match(&call.args),
                        _ => ExtFunctionResult::Return(MontyObject::None),
                    };
                    progress = call
                        .resume(ext_result, PrintWriter::CollectString(&mut stdout))
                        .expect("resume failed");
                }
                RunProgress::NameLookup(lookup) => {
                    progress = lookup
                        .resume(
                            NameLookupResult::Undefined,
                            PrintWriter::CollectString(&mut stdout),
                        )
                        .expect("name lookup resume failed");
                }
                _ => panic!("Unexpected RunProgress variant in test"),
            }
        }
    }

    fn eval_python_bool(expr: &str) -> bool {
        // basic_mode.py has no helper section (the v3 script is a single
        // `def main`), so the slice before `def main` is just the comment
        // header — harmless to prepend to a standalone snippet.
        let helpers_end = DEFAULT_ORCHESTRATOR
            .find("\ndef main(")
            .unwrap_or(DEFAULT_ORCHESTRATOR.len());
        let helpers = &DEFAULT_ORCHESTRATOR[..helpers_end]; // safety: find() returns a char boundary on this ASCII-only constant

        let code = format!("{helpers}\nFINAL({expr})");
        match run_python_final(code) {
            MontyObject::Bool(v) => v,
            other => panic!("Expected bool, got: {other:?}"),
        }
    }

    /// Run a Python program (with orchestrator helpers in scope) that ends
    /// with `FINAL(int_expr)` and return the integer value.
    fn eval_python_int(program: &str) -> i64 {
        let helpers_end = DEFAULT_ORCHESTRATOR
            .find("\ndef main(")
            .unwrap_or(DEFAULT_ORCHESTRATOR.len());
        let helpers = &DEFAULT_ORCHESTRATOR[..helpers_end];

        let code = format!("{helpers}\n{program}");
        match run_python_final(code) {
            MontyObject::Int(v) => v,
            other => panic!("Expected int, got: {other:?}"),
        }
    }

    // ── __regex_match__ host function reachability ───────────────

    #[test]
    fn regex_match_host_function_is_callable_from_monty() {
        // Regression test for PR #1736 review (serrrfirat, 3059161877):
        // verify that Monty's NameLookup + FunctionCall dispatch actually
        // reaches `handle_regex_match` when default.py calls
        // `__regex_match__(...)`. If Monty ever starts resolving the name
        // before the call, this test will fail with a NameError.
        assert!(eval_python_bool(
            r#"bool(__regex_match__("abc", "xxabcxx"))"#
        ));
        assert!(!eval_python_bool(
            r#"bool(__regex_match__("zzz", "xxabcxx"))"#
        ));
        // Invalid pattern should return false silently (the host function
        // swallows the compile error).
        assert!(!eval_python_bool(r#"bool(__regex_match__("[", "abc"))"#));
    }

    #[tokio::test]
    async fn load_orchestrator_without_store_returns_default() {
        let (code, version) = load_orchestrator(None, ProjectId::new(), true).await;
        assert_eq!(version, 0);
        assert!(code.contains("def main"));
        assert!(code.contains("host.resolve_intent"));
    }

    #[tokio::test]
    async fn load_orchestrator_with_runtime_version() {
        let project_id = ProjectId::new();
        let mut doc = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            ORCHESTRATOR_TITLE,
            "custom_orchestrator_code()",
        )
        .with_tags(vec![ORCHESTRATOR_TAG.to_string()]);
        doc.metadata = serde_json::json!({"version": 1});

        let store = Arc::new(crate::tests::InMemoryStore::with_docs(vec![doc]));
        let (code, version) =
            load_orchestrator(Some(&(store as Arc<dyn Store>)), project_id, true).await;
        assert_eq!(version, 1);
        assert!(code.contains("custom_orchestrator_code"));
    }

    #[tokio::test]
    async fn load_orchestrator_picks_highest_version() {
        let project_id = ProjectId::new();
        let mut doc_v1 = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            ORCHESTRATOR_TITLE,
            "v1_code()",
        )
        .with_tags(vec![ORCHESTRATOR_TAG.to_string()]);
        doc_v1.metadata = serde_json::json!({"version": 1});

        let mut doc_v3 = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            ORCHESTRATOR_TITLE,
            "v3_code()",
        )
        .with_tags(vec![ORCHESTRATOR_TAG.to_string()]);
        doc_v3.metadata = serde_json::json!({"version": 3});

        let mut doc_v2 = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            ORCHESTRATOR_TITLE,
            "v2_code()",
        )
        .with_tags(vec![ORCHESTRATOR_TAG.to_string()]);
        doc_v2.metadata = serde_json::json!({"version": 2});

        let store = Arc::new(crate::tests::InMemoryStore::with_docs(vec![
            doc_v1, doc_v3, doc_v2,
        ]));
        let (code, version) =
            load_orchestrator(Some(&(store as Arc<dyn Store>)), project_id, true).await;
        assert_eq!(version, 3);
        assert!(code.contains("v3_code"));
    }

    #[tokio::test]
    async fn rollback_after_max_failures() {
        let project_id = ProjectId::new();

        // Create v2 orchestrator
        let mut doc_v2 = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            ORCHESTRATOR_TITLE,
            "v2_buggy()",
        )
        .with_tags(vec![ORCHESTRATOR_TAG.to_string()]);
        doc_v2.metadata = serde_json::json!({"version": 2});

        // Create v1 orchestrator (fallback)
        let mut doc_v1 = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            ORCHESTRATOR_TITLE,
            "v1_stable()",
        )
        .with_tags(vec![ORCHESTRATOR_TAG.to_string()]);
        doc_v1.metadata = serde_json::json!({"version": 1});

        // Create failure tracker showing v2 has 3 failures
        let tracker = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            FAILURE_TRACKER_TITLE,
            r#"{"version": 2, "count": 3}"#,
        )
        .with_tags(vec!["orchestrator_meta".to_string()]);

        let store = Arc::new(crate::tests::InMemoryStore::with_docs(vec![
            doc_v2, doc_v1, tracker,
        ]));
        let (code, version) =
            load_orchestrator(Some(&(store as Arc<dyn Store>)), project_id, true).await;

        // Should skip v2 (too many failures) and load v1
        assert_eq!(version, 1);
        assert!(code.contains("v1_stable"));
    }

    #[tokio::test]
    async fn rollback_to_default_when_all_versions_fail() {
        let project_id = ProjectId::new();

        // Single version with 3 failures
        let mut doc_v1 = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            ORCHESTRATOR_TITLE,
            "v1_broken()",
        )
        .with_tags(vec![ORCHESTRATOR_TAG.to_string()]);
        doc_v1.metadata = serde_json::json!({"version": 1});

        let tracker = MemoryDoc::new(
            project_id,
            "system",
            DocType::Note,
            FAILURE_TRACKER_TITLE,
            r#"{"version": 1, "count": 5}"#,
        )
        .with_tags(vec!["orchestrator_meta".to_string()]);

        let store = Arc::new(crate::tests::InMemoryStore::with_docs(vec![
            doc_v1, tracker,
        ]));
        let (code, version) =
            load_orchestrator(Some(&(store as Arc<dyn Store>)), project_id, true).await;

        // Should fall back to compiled-in default (v0)
        assert_eq!(version, 0);
        assert!(code.contains("def main"));
    }

    #[tokio::test]
    async fn record_and_reset_failures() {
        let project_id = ProjectId::new();
        let store: Arc<dyn Store> = Arc::new(crate::tests::InMemoryStore::with_docs(vec![]));

        // Record 3 failures
        record_orchestrator_failure(&store, project_id, 2).await;
        record_orchestrator_failure(&store, project_id, 2).await;
        record_orchestrator_failure(&store, project_id, 2).await;

        let docs = store.list_shared_memory_docs(project_id).await.unwrap();
        let count = load_failure_count(&docs);
        assert_eq!(count, 3);

        // Reset
        reset_orchestrator_failures(&store, project_id).await;
        let docs = store.list_shared_memory_docs(project_id).await.unwrap();
        let count = load_failure_count(&docs);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn failure_count_resets_on_new_version() {
        let project_id = ProjectId::new();
        let store: Arc<dyn Store> = Arc::new(crate::tests::InMemoryStore::with_docs(vec![]));

        // Record failures for version 1
        record_orchestrator_failure(&store, project_id, 1).await;
        record_orchestrator_failure(&store, project_id, 1).await;

        // Switch to version 2 — count should reset to 1
        record_orchestrator_failure(&store, project_id, 2).await;

        let docs = store.list_shared_memory_docs(project_id).await.unwrap();
        let count = load_failure_count(&docs);
        assert_eq!(count, 1);
    }

    #[test]
    fn normalize_pause_outcome_transitions_thread_to_waiting() {
        let mut thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        thread.transition_to(ThreadState::Running, None).unwrap();

        let outcome = ThreadOutcome::GatePaused {
            gate_name: "approval".into(),
            action_name: "shell".into(),
            call_id: "call-1".into(),
            parameters: serde_json::json!({"cmd":"ls"}),
            resume_kind: crate::gate::ResumeKind::Approval { allow_always: true },
            resume_output: None,
            paused_lease: None,
        };
        normalize_pause_outcome(&mut thread, &outcome).unwrap();
        assert_eq!(thread.state, ThreadState::Waiting);
    }

    #[test]
    fn parse_outcome_completed() {
        let result = serde_json::json!({"outcome": "completed", "response": "Hello!"});
        let outcome = parse_outcome(&result);
        assert!(matches!(outcome, ThreadOutcome::Completed { response: Some(r) } if r == "Hello!"));
    }

    #[test]
    fn parse_outcome_failed() {
        let result = serde_json::json!({"outcome": "failed", "error": "boom"});
        let outcome = parse_outcome(&result);
        assert!(matches!(outcome, ThreadOutcome::Failed { error, .. } if error == "boom"));
    }

    #[test]
    fn parse_outcome_gate_paused() {
        let lease = crate::types::capability::CapabilityLease {
            id: crate::types::capability::LeaseId::new(),
            thread_id: crate::types::thread::ThreadId::new(),
            capability_name: "test-capability".into(),
            granted_actions: crate::types::capability::GrantedActions::Specific(vec![
                "shell".into(),
            ]),
            granted_at: chrono::Utc::now(),
            expires_at: None,
            max_uses: Some(1),
            uses_remaining: Some(1),
            revoked: false,
            revoked_reason: None,
        };
        let result = serde_json::json!({
            "outcome": "gate_paused",
            "gate_name": "approval",
            "action_name": "shell",
            "call_id": "abc",
            "parameters": {"cmd": "rm -rf /"},
            "resume_kind": {"Approval": {"allow_always": true}},
            "paused_lease": lease,
        });
        let outcome = parse_outcome(&result);
        assert!(matches!(
            outcome,
            ThreadOutcome::GatePaused {
                action_name,
                paused_lease: Some(_),
                ..
            } if action_name == "shell"
        ));
    }

    #[test]
    fn parse_outcome_max_iterations() {
        let result = serde_json::json!({"outcome": "max_iterations"});
        let outcome = parse_outcome(&result);
        assert!(matches!(outcome, ThreadOutcome::MaxIterations));
    }

    #[test]
    fn parse_outcome_stopped() {
        let result = serde_json::json!({"outcome": "stopped"});
        let outcome = parse_outcome(&result);
        assert!(matches!(outcome, ThreadOutcome::Stopped));
    }

    /// Regression test for chtugha/brassclaw#2084 — drives the
    /// `list_skills_from_store` caller end-to-end (not just the
    /// `list_skills_global` helper). This is the caller-level test required by
    /// `.claude/rules/testing.md` ("Test Through the Caller, Not Just the
    /// Helper"): a future regression that reverts the MemoryDoc fallback back to
    /// `list_memory_docs_with_shared(thread.project_id, &thread.user_id)` would
    /// slip past a helper-only unit test but must fail this one, because the
    /// shared skill lives in a different project than the caller's thread.
    #[tokio::test]
    async fn list_skills_from_store_returns_shared_skills_from_other_projects() {
        use crate::types::shared_owner_id;
        use crate::types::thread::{ThreadConfig, ThreadType};

        // project_a: where alice's thread runs.
        // project_b: where the admin installed a shared skill.
        let project_a = ProjectId::new();
        let project_b = ProjectId::new();

        let shared_skill = MemoryDoc::new(
            project_b,
            shared_owner_id(),
            DocType::Skill,
            "skill:admin-installed",
            "shared content",
        );
        let alice_skill = MemoryDoc::new(
            project_a,
            "alice",
            DocType::Skill,
            "skill:alice-owned",
            "alice content",
        );
        // A non-skill doc in alice's project must not leak into the result.
        let alice_note = MemoryDoc::new(
            project_a,
            "alice",
            DocType::Note,
            "note:scratch",
            "note body",
        );

        let store: Arc<dyn Store> = Arc::new(crate::tests::InMemoryStore::with_docs(vec![
            shared_skill.clone(),
            alice_skill.clone(),
            alice_note,
        ]));

        let thread = Thread::new(
            "test goal",
            ThreadType::Foreground,
            project_a,
            "alice",
            ThreadConfig::default(),
        );

        let skills = list_skills_from_store(&store, &thread).await;

        let titles: Vec<&str> = skills
            .iter()
            .filter_map(|v| v.get("title").and_then(|t| t.as_str()))
            .collect();

        assert!(
            titles.contains(&"skill:admin-installed"),
            "shared skill from project_b must be visible to alice's thread in project_a — got {titles:?}"
        );
        assert!(
            titles.contains(&"skill:alice-owned"),
            "alice's own skill must be visible — got {titles:?}"
        );
        assert!(
            !titles.contains(&"note:scratch"),
            "non-skill docs must be filtered out — got {titles:?}"
        );
        assert_eq!(
            skills.len(),
            2,
            "expected exactly 2 skills (shared + alice), got {}: {titles:?}",
            skills.len()
        );
    }

    /// `handle_list_skills` thin-calls `ComponentPort::list_skills`; with no
    /// port wired (`None`) it degrades to an empty list (C.6 slice 4c-prep).
    #[tokio::test]
    async fn handle_list_skills_no_port_returns_empty_list() {
        let thread = phase_f7_thread("deploy now");
        let result = handle_list_skills(&[], &thread, None).await;
        let ExtFunctionResult::Return(obj) = result else {
            panic!("handle_list_skills did not return a value");
        };
        let json = monty_to_json(&obj);
        let arr = json
            .as_array()
            .expect("handle_list_skills must return a JSON array");
        assert!(arr.is_empty(), "no-port path must return an empty list");
    }

    /// `handle_resolve_intent` thin-calls `ComponentPort::resolve_intent`; with
    /// no port wired (`None`) it degrades to `{"status":"no_match"}` (the
    /// orchestrator's Non-Matching-Mode trigger), and a missing `user_input`
    /// arg degrades to `{"status":"no_match","error":"missing user_input"}`.
    #[tokio::test]
    async fn handle_resolve_intent_no_port_returns_no_match() {
        let thread = phase_f7_thread("deploy now");

        // Missing user_input arg.
        let result = handle_resolve_intent(&[], &[], &thread, None).await;
        let ExtFunctionResult::Return(obj) = result else {
            panic!("handle_resolve_intent did not return a value");
        };
        let json = monty_to_json(&obj);
        assert_eq!(json["status"], serde_json::json!("no_match"));
        assert_eq!(json["error"], serde_json::json!("missing user_input"));

        // Present user_input, no port.
        let args = vec![MontyObject::String("deploy the thing".into())];
        let result = handle_resolve_intent(&args, &[], &thread, None).await;
        let ExtFunctionResult::Return(obj) = result else {
            panic!("handle_resolve_intent did not return a value");
        };
        let json = monty_to_json(&obj);
        assert_eq!(json["status"], serde_json::json!("no_match"));
    }

    /// No-op effect executor — only consulted for
    /// `available_actions(...)`, which we satisfy with an empty list.
    struct NoopEffects;

    #[async_trait::async_trait]
    impl EffectExecutor for NoopEffects {
        async fn execute_action(
            &self,
            _: &str,
            _: serde_json::Value,
            _: &crate::types::capability::CapabilityLease,
            _: &ThreadExecutionContext,
        ) -> Result<crate::types::step::ActionResult, EngineError> {
            Ok(crate::types::step::ActionResult {
                call_id: String::new(),
                action_name: String::new(),
                output: serde_json::json!({}),
                is_error: false,
                duration: std::time::Duration::from_millis(1),
            })
        }

        async fn available_actions(
            &self,
            _: &[crate::types::capability::CapabilityLease],
            _: &ThreadExecutionContext,
        ) -> Result<Vec<crate::types::capability::ActionDef>, EngineError> {
            Ok(vec![])
        }

        async fn available_capabilities(
            &self,
            _: &[crate::types::capability::CapabilityLease],
            _: &ThreadExecutionContext,
        ) -> Result<Vec<crate::types::capability::CapabilitySummary>, EngineError> {
            Ok(vec![])
        }
    }

    // ── Python ↔ Rust ActionCall round-trip ───────────────────────────────
    //
    // Regression tests for the orphaned-tool-result bug. The Python
    // orchestrator stores `action_calls` on assistant messages using the
    // shape `{name, call_id, params}`, but the canonical Rust `ActionCall`
    // uses `{action_name, id, parameters}`. Without the explicit
    // `PythonActionCall` interchange type, `serde_json::from_value` would
    // silently fail (`.ok()` swallows the error) and the Python-shaped
    // assistant message would be parsed back as a plain assistant message
    // with no tool calls, causing every subsequent ActionResult to be
    // detected as orphaned by `sanitize_tool_messages` in the host crate.

    #[test]
    fn python_action_call_round_trips_through_serde() {
        let original = ActionCall {
            id: "call_abc123".to_string(),
            action_name: "google_drive_tool".to_string(),
            parameters: serde_json::json!({"query": "expenses"}),
        };

        let python_json = serde_json::to_value(PythonActionCall::from(&original))
            .expect("PythonActionCall must serialize");
        // Python-friendly field names — match what default.py reads.
        assert_eq!(python_json["name"], "google_drive_tool");
        assert_eq!(python_json["call_id"], "call_abc123");
        assert_eq!(
            python_json["params"],
            serde_json::json!({"query": "expenses"})
        );

        let parsed: PythonActionCall =
            serde_json::from_value(python_json).expect("must deserialize");
        let round_tripped: ActionCall = parsed.into();
        assert_eq!(round_tripped.id, original.id);
        assert_eq!(round_tripped.action_name, original.action_name);
        assert_eq!(round_tripped.parameters, original.parameters);
    }

    #[test]
    fn action_calls_to_python_json_uses_python_field_names() {
        let calls = vec![
            ActionCall {
                id: "call_1".to_string(),
                action_name: "notion_notion_search".to_string(),
                parameters: serde_json::json!({"query": "name"}),
            },
            ActionCall {
                id: "call_2".to_string(),
                action_name: "google_drive_tool".to_string(),
                parameters: serde_json::json!({"action": "list"}),
            },
        ];
        let json = action_calls_to_python_json(&calls);
        assert_eq!(json.len(), 2);
        assert_eq!(json[0]["name"], "notion_notion_search");
        assert_eq!(json[0]["call_id"], "call_1");
        assert_eq!(json[1]["name"], "google_drive_tool");
        assert_eq!(json[1]["call_id"], "call_2");
    }

    #[test]
    fn python_json_to_action_calls_parses_python_field_names() {
        // The exact shape default.py produces (and stores on assistant
        // messages via `append_message(..., action_calls=calls)`).
        let python_json = serde_json::json!([
            {"name": "notion_notion_search", "call_id": "call_xyz", "params": {"q": "foo"}},
            {"name": "google_drive_tool", "call_id": "call_abc", "params": {"action": "list"}},
        ]);
        let parsed = python_json_to_action_calls(&python_json).expect("must parse");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].action_name, "notion_notion_search");
        assert_eq!(parsed[0].id, "call_xyz");
        assert_eq!(parsed[0].parameters, serde_json::json!({"q": "foo"}));
        assert_eq!(parsed[1].action_name, "google_drive_tool");
        assert_eq!(parsed[1].id, "call_abc");
    }

    #[test]
    fn python_json_to_action_calls_rejects_canonical_field_names() {
        // Sanity check: the parser is strict about Python field names.
        // If `default.py` ever changes the shape, the test must catch it.
        let canonical_json = serde_json::json!([
            {"action_name": "search", "id": "call_x", "parameters": {}}
        ]);
        // Missing "name", "call_id", "params" → returns None.
        assert!(python_json_to_action_calls(&canonical_json).is_none());
    }

    #[test]
    fn summarize_action_calls_for_log_does_not_leak_user_pii() {
        // The whole point of this helper is that the warn log path on a
        // shape-drift failure must NOT dump tool parameters (which can
        // contain user PII like search queries, file names, email content)
        // into log aggregation systems. The summary should expose only
        // structural information: array length and the keys of the first
        // entry. The keys themselves are static (`name`, `call_id`,
        // `params`), not user data.
        let pii_value = serde_json::json!([
            {
                "name": "google_drive_tool",
                "call_id": "call_xyz",
                "params": {
                    "query": "salary spreadsheet for joe",
                    "secret_token": "very-sensitive-token-do-not-log"
                }
            },
            {
                "name": "gmail",
                "call_id": "call_abc",
                "params": {
                    "subject": "private message about layoffs"
                }
            }
        ]);
        let summary = summarize_action_calls_for_log(&pii_value);

        // Structural info present.
        assert!(summary.contains("array of 2 entries"));
        assert!(summary.contains("call_id"));
        assert!(summary.contains("name"));
        assert!(summary.contains("params"));

        // PII fields and their values must NOT appear.
        assert!(
            !summary.contains("salary"),
            "summary must not leak user PII from params: {summary}"
        );
        assert!(
            !summary.contains("very-sensitive-token"),
            "summary must not leak credential-shaped values: {summary}"
        );
        assert!(
            !summary.contains("layoffs"),
            "summary must not leak free-text content: {summary}"
        );
        assert!(
            !summary.contains("google_drive_tool"),
            "summary must not leak the tool name itself (could expose intent): {summary}"
        );
    }

    #[test]
    fn summarize_action_calls_for_log_handles_edge_cases() {
        assert_eq!(
            summarize_action_calls_for_log(&serde_json::json!([])),
            "empty array"
        );
        assert!(
            summarize_action_calls_for_log(&serde_json::json!("not an array")).contains("string")
        );
        assert!(
            summarize_action_calls_for_log(&serde_json::json!({"foo": "bar"})).contains("object")
        );
        assert!(summarize_action_calls_for_log(&serde_json::json!(null)).contains("null"));
    }

    /// Caller-level regression test: feeds `json_to_thread_messages` the
    /// exact JSON shape that `default.py` produces for an assistant message
    /// with tool calls followed by tool results, and asserts that the
    /// resulting `ThreadMessage`s preserve the `action_calls` ↔
    /// `action_call_id` linkage. Without the `PythonActionCall` parser the
    /// assistant message would come back with `action_calls = None` and
    /// every following ActionResult would look orphaned to the bridge.
    #[test]
    fn json_to_thread_messages_preserves_action_calls_from_python_orchestrator() {
        // This is the literal shape `default.py` writes into
        // `state["working_messages"]` after a Tier 0 step:
        //
        //   append_message(working_messages, "Assistant", "...", action_calls=calls)
        //   append_message(working_messages, "ActionResult", "...", action_name=..., action_call_id=...)
        //
        // where `calls` came from the LLM response and has shape
        // `[{"name": ..., "call_id": ..., "params": ...}]`.
        let working_messages = serde_json::json!([
            {"role": "User", "content": "search in notion for my name"},
            {
                "role": "Assistant",
                "content": "",
                "action_calls": [
                    {
                        "name": "notion_notion_search",
                        "call_id": "call_xyz",
                        "params": {"query": "Illia"}
                    }
                ]
            },
            {
                "role": "ActionResult",
                "content": "found 3 results",
                "action_name": "notion_notion_search",
                "action_call_id": "call_xyz"
            }
        ]);

        let messages = json_to_thread_messages(&working_messages).expect("must parse");
        assert_eq!(messages.len(), 3);

        // The assistant message MUST have action_calls populated, with
        // matching call_id. If this assertion fails, the bridge layer
        // will treat the following ActionResult as orphaned and rewrite
        // it as a user message — losing the model's ability to reason
        // about prior tool output.
        let assistant = &messages[1];
        assert_eq!(
            assistant.role,
            crate::types::message::MessageRole::Assistant
        );
        let calls = assistant
            .action_calls
            .as_ref()
            .expect("assistant message must carry action_calls after round-trip");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_xyz");
        assert_eq!(calls[0].action_name, "notion_notion_search");
        assert_eq!(calls[0].parameters, serde_json::json!({"query": "Illia"}));

        // The ActionResult must reference the same call_id so the bridge
        // can pair them.
        let result = &messages[2];
        assert_eq!(
            result.role,
            crate::types::message::MessageRole::ActionResult
        );
        assert_eq!(result.action_call_id.as_deref(), Some("call_xyz"));
        assert_eq!(result.action_name.as_deref(), Some("notion_notion_search"));
    }

    /// Regression for the gate-resume / bootstrap path: when a thread
    /// resumes after approval or auth, `build_orchestrator_inputs`
    /// serializes `thread.internal_messages` into the bootstrap context
    /// that Python reads into `working_messages`. If `action_calls` is
    /// serialized with canonical `ActionCall` field names (`action_name`,
    /// `id`, `parameters`) instead of the Python interchange names
    /// (`name`, `call_id`, `params`), the next `__llm_complete__` call
    /// passes them back through `json_to_thread_messages` which fails
    /// with "missing field `name`" and orphans every subsequent tool
    /// result.
    ///
    /// This test simulates the full round-trip: build a `ThreadMessage`
    /// with action_calls → serialize through `build_orchestrator_inputs`'s
    /// exact serialization pattern → parse back through
    /// `json_to_thread_messages` → assert the calls survive. If anyone
    /// adds a THIRD serialization path in the future and uses canonical
    /// names, this test documents the pattern they should follow.
    #[test]
    fn bootstrap_context_action_calls_round_trip_through_python_interchange() {
        // Build a thread message the way the engine does: an assistant
        // message with action_calls in canonical ActionCall format (the
        // shape stored in the DB / internal_messages).
        let msg = ThreadMessage::assistant_with_actions(
            Some("I'll search for that".to_string()),
            vec![ActionCall {
                id: "call_resume_test".to_string(),
                action_name: "google_drive_tool".to_string(),
                parameters: serde_json::json!({"query": "budget"}),
            }],
        );

        // Serialize through the SAME pattern `build_orchestrator_inputs`
        // uses. This is the exact code path that was broken before the
        // fix — it was using `"action_calls": m.action_calls` which
        // produced canonical field names.
        let calls_json = msg
            .action_calls
            .as_ref()
            .map(|calls| serde_json::Value::Array(action_calls_to_python_json(calls)));
        let serialized = serde_json::json!([{
            "role": "Assistant",
            "content": msg.content,
            "action_name": msg.action_name,
            "action_call_id": msg.action_call_id,
            "action_calls": calls_json,
        }]);

        // Parse back through the same path Python's working_messages
        // takes when it calls __llm_complete__.
        let parsed = json_to_thread_messages(&serialized).expect("must parse");
        assert_eq!(parsed.len(), 1);

        let assistant = &parsed[0];
        let calls = assistant.action_calls.as_ref().expect(
            "bootstrap context action_calls must survive the round-trip. \
                 If this fails, a serialization path is using canonical ActionCall \
                 field names instead of PythonActionCall interchange names.",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_resume_test");
        assert_eq!(calls[0].action_name, "google_drive_tool");
        assert_eq!(calls[0].parameters, serde_json::json!({"query": "budget"}));
    }

    /// Negative regression: verify that canonical ActionCall field names
    /// do NOT round-trip. If this test ever PASSES, it means someone
    /// added `#[serde(rename)]` to ActionCall or changed the parser to
    /// accept both formats — which is fine, but the PythonActionCall
    /// interchange type can then be removed. This test documents the
    /// current contract: canonical names are rejected by the parser.
    #[test]
    fn canonical_action_call_field_names_do_not_round_trip() {
        let serialized_with_canonical_names = serde_json::json!([{
            "role": "Assistant",
            "content": "",
            "action_calls": [{
                "action_name": "search",
                "id": "call_x",
                "parameters": {}
            }],
        }]);
        let parsed =
            json_to_thread_messages(&serialized_with_canonical_names).expect("messages parse");
        // The assistant message should have NO action_calls because the
        // parser rejects canonical field names.
        assert!(
            parsed[0].action_calls.is_none(),
            "canonical ActionCall field names must NOT parse as action_calls. \
             If this assertion fails, the PythonActionCall interchange type \
             is no longer needed — either remove it or update the contract."
        );
    }

    /// Regression: `action_calls: null` is Python's legitimate "this
    /// message has no tool calls" signal (text-only response). Before the
    /// null filter, `python_json_to_action_calls` would fire a warn log
    /// with "invalid type: null, expected a sequence" on every text-only
    /// assistant message — a false alarm that masked real drift issues.
    #[test]
    fn json_to_thread_messages_handles_null_action_calls_gracefully() {
        let messages = serde_json::json!([
            {
                "role": "Assistant",
                "content": "Here is your answer.",
                "action_calls": null
            }
        ]);
        let parsed = json_to_thread_messages(&messages).expect("must parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].role,
            crate::types::message::MessageRole::Assistant
        );
        assert_eq!(parsed[0].content, "Here is your answer.");
        assert!(
            parsed[0].action_calls.is_none(),
            "null action_calls must produce None, not a parse error"
        );
    }

    /// Verify that messages WITHOUT the action_calls key at all (the most
    /// common case for text responses) also parse correctly — this is the
    /// baseline that the null-filtering regression test extends.
    #[test]
    fn json_to_thread_messages_handles_absent_action_calls() {
        let messages = serde_json::json!([
            {"role": "Assistant", "content": "Just text, no tools."}
        ]);
        let parsed = json_to_thread_messages(&messages).expect("must parse");
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].action_calls.is_none());
    }

    /// Empty action_calls array is valid (LLM decided not to call any
    /// tools this turn but the response still has the array field). Must
    /// produce `Some(vec![])`, not `None`.
    #[test]
    fn json_to_thread_messages_handles_empty_action_calls_array() {
        let messages = serde_json::json!([
            {
                "role": "Assistant",
                "content": "No tools needed.",
                "action_calls": []
            }
        ]);
        let parsed = json_to_thread_messages(&messages).expect("must parse");
        assert_eq!(parsed.len(), 1);
        let calls = parsed[0]
            .action_calls
            .as_ref()
            .expect("empty array should produce Some(vec![])");
        assert!(calls.is_empty());
    }

    // ── Consecutive action error counting (issue #2325) ──────────
    //
    // The run_loop tracks `consecutive_action_errors` for Tier 0 (structured
    // action calls). These tests exercise the counting logic extracted from
    // run_loop into small Python snippets that simulate batch outcomes.

    #[test]
    fn action_errors_increment_when_all_actions_fail() {
        // Simulate 3 consecutive batches where all actions fail.
        let count = eval_python_int(
            r#"
consecutive_action_errors = 0
for _ in range(3):
    batch_error_count = 2
    batch_success_count = 0
    if batch_success_count > 0:
        consecutive_action_errors = 0
    elif batch_error_count > 0:
        consecutive_action_errors += 1
FINAL(consecutive_action_errors)
"#,
        );
        assert_eq!(count, 3);
    }

    #[test]
    fn action_errors_reset_when_any_action_succeeds() {
        // 2 all-fail batches, then 1 batch with a success => resets to 0.
        let count = eval_python_int(
            r#"
consecutive_action_errors = 0
for batch in [(0, 2), (0, 1), (1, 1)]:
    batch_success_count = batch[0]
    batch_error_count = batch[1]
    if batch_success_count > 0:
        consecutive_action_errors = 0
    elif batch_error_count > 0:
        consecutive_action_errors += 1
FINAL(consecutive_action_errors)
"#,
        );
        assert_eq!(count, 0);
    }

    #[test]
    fn action_errors_partial_success_resets_counter() {
        // A batch with mixed results (some succeed, some fail) should reset.
        let count = eval_python_int(
            r#"
consecutive_action_errors = 5
batch_success_count = 1
batch_error_count = 3
if batch_success_count > 0:
    consecutive_action_errors = 0
elif batch_error_count > 0:
    consecutive_action_errors += 1
FINAL(consecutive_action_errors)
"#,
        );
        assert_eq!(count, 0);
    }

    /// Regression: `max_consecutive_errors` arrives as `null` when the Rust
    /// caller passes `Option::None`. Python's `dict.get(key, default)` returns
    /// the explicit `None`, not the default, so `None + 2` used to blow up in
    /// the error-gating branch on the very first failed action call. The
    /// orchestrator now coalesces `None` to a sentinel; this test pins that
    /// behavior.
    #[test]
    fn action_errors_tolerate_null_max_consecutive_errors() {
        let result = eval_python_int(
            r#"
max_consecutive_errors = None
if max_consecutive_errors is None:
    max_consecutive_errors = 10**9
consecutive_action_errors = 1
nudge = False
failed = False
if consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors + 2:
    failed = True
elif consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors:
    nudge = True
# 0 = no nudge / no failure (expected with None = no-limit sentinel)
if failed:
    FINAL(2)
elif nudge:
    FINAL(1)
else:
    FINAL(0)
"#,
        );
        assert_eq!(
            result, 0,
            "None max_consecutive_errors must behave as no-limit, not crash"
        );
    }

    #[test]
    fn action_errors_nudge_injected_at_threshold() {
        // When consecutive_action_errors reaches max_consecutive_errors,
        // a nudge message should be appended. We simulate the branching
        // logic and check whether a nudge would fire.
        // Returns 1 if nudge fires (not failure), 0 otherwise.
        let result = eval_python_int(
            r#"
max_consecutive_errors = 5
consecutive_action_errors = 5
nudge = False
failed = False
if consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors + 2:
    failed = True
elif consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors:
    nudge = True
if nudge and not failed:
    FINAL(1)
else:
    FINAL(0)
"#,
        );
        assert_eq!(result, 1, "nudge should fire at threshold");
    }

    #[test]
    fn action_errors_no_nudge_below_threshold() {
        // Returns 1 if nudge fires, 0 if not.
        let result = eval_python_int(
            r#"
max_consecutive_errors = 5
consecutive_action_errors = 4
nudge = False
failed = False
if consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors + 2:
    failed = True
elif consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors:
    nudge = True
if nudge:
    FINAL(1)
else:
    FINAL(0)
"#,
        );
        assert_eq!(result, 0, "nudge should not fire below threshold");
    }

    #[test]
    fn action_errors_failure_at_threshold_plus_two() {
        // At max_consecutive_errors + 2, the thread should transition to failed.
        // Returns 1 if failed, 0 if not.
        let result = eval_python_int(
            r#"
max_consecutive_errors = 5
consecutive_action_errors = 7
failed = False
if consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors + 2:
    failed = True
if failed:
    FINAL(1)
else:
    FINAL(0)
"#,
        );
        assert_eq!(result, 1, "should fail at threshold + 2");
    }

    #[test]
    fn action_errors_nudge_at_threshold_not_failure() {
        // At exactly max_consecutive_errors + 1, we get a nudge but not failure.
        let result = eval_python_int(
            r#"
max_consecutive_errors = 5
consecutive_action_errors = 6
nudge = False
failed = False
if consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors + 2:
    failed = True
elif consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors:
    nudge = True
# Return 0=nothing, 1=nudge, 2=failed
if failed:
    FINAL(2)
elif nudge:
    FINAL(1)
else:
    FINAL(0)
"#,
        );
        assert_eq!(result, 1, "should nudge at threshold + 1, not fail");
    }

    #[test]
    fn action_errors_none_limit_skips_check_without_typeerror() {
        // Regression: when max_consecutive_errors is None (meaning "no limit"),
        // the arithmetic `max_consecutive_errors + 2` used to crash with
        // TypeError on the first action error. The guard must short-circuit
        // on None and leave both the nudge and failure branches untaken.
        let result = eval_python_int(
            r#"
max_consecutive_errors = None
consecutive_action_errors = 1
nudge = False
failed = False
if max_consecutive_errors is not None and consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors + 2:
    failed = True
elif max_consecutive_errors is not None and consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors:
    nudge = True
# Return 0=nothing, 1=nudge, 2=failed
if failed:
    FINAL(2)
elif nudge:
    FINAL(1)
else:
    FINAL(0)
"#,
        );
        assert_eq!(result, 0, "None limit should disable the guard entirely");
    }

    #[test]
    fn code_errors_none_limit_skips_failure_check() {
        // Regression: same None-guard for the code-error branch at line 660.
        // consecutive_errors = TEST_CONSECUTIVE_ERRORS; None limit must not trigger.
        let result = eval_python_int(&format!(
            r#"
max_consecutive_errors = None
consecutive_errors = {ce}
failed = False
if max_consecutive_errors is not None and consecutive_errors >= max_consecutive_errors:
    failed = True
if failed:
    FINAL(1)
else:
    FINAL(0)
"#,
            ce = TEST_CONSECUTIVE_ERRORS,
        ));
        assert_eq!(
            result, 0,
            "None limit should not trigger failure regardless of consecutive_errors"
        );
    }

    #[test]
    fn action_error_prefix_added_to_error_output() {
        // Verify that [ACTION FAILED] prefix is prepended to error outputs.
        // Returns 1 if prefix present, 0 if not.
        let result = eval_python_int(
            r#"
r = {"action_name": "http", "output": "connection refused", "is_error": True}
output = r.get("output")
output_str = str(output) if output is not None else "[no output]"
if r.get("is_error"):
    output_str = "[ACTION FAILED] " + output_str
if output_str.startswith("[ACTION FAILED]"):
    FINAL(1)
else:
    FINAL(0)
"#,
        );
        assert_eq!(result, 1, "error outputs must get [ACTION FAILED] prefix");
    }

    #[test]
    fn action_error_skipped_calls_count_as_errors() {
        // When a call has no result (r is None), it should count as an error.
        let count = eval_python_int(
            r#"
batch_error_count = 0
batch_success_count = 0
r = None
if r is not None:
    if r.get("is_error"):
        batch_error_count += 1
    else:
        batch_success_count += 1
else:
    batch_error_count += 1
FINAL(batch_error_count)
"#,
        );
        assert_eq!(count, 1, "skipped calls must count as batch errors");
    }

    #[test]
    fn checkpoint_includes_consecutive_action_errors() {
        // Test that handle_save_checkpoint persists consecutive_action_errors
        // in the thread metadata.
        let mut thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        thread.transition_to(ThreadState::Running, None).unwrap();

        let state = json_to_monty(&serde_json::json!({}));
        let counters = json_to_monty(&serde_json::json!({
            "nudge_count": 0,
            "consecutive_errors": 1,
            "consecutive_action_errors": 4,
            "compaction_count": 2,
        }));

        handle_save_checkpoint(&[state, counters], &[], &mut thread);

        let checkpoint = thread
            .metadata
            .get("runtime_checkpoint")
            .expect("checkpoint must exist");
        assert_eq!(
            checkpoint
                .get("consecutive_action_errors")
                .and_then(|v| v.as_u64()),
            Some(4),
            "consecutive_action_errors must be persisted in checkpoint"
        );
        assert_eq!(
            checkpoint
                .get("consecutive_errors")
                .and_then(|v| v.as_u64()),
            Some(1),
        );
        assert_eq!(
            checkpoint.get("compaction_count").and_then(|v| v.as_u64()),
            Some(2),
        );
    }

    /// Regression test: every assistant tool_call must have a matching
    /// ActionResult after parsing. If an ActionResult is missing, the LLM
    /// API rejects with "No tool output found for function call <id>".
    ///
    /// This was the root cause of the HTTP 400 from the OpenAI Codex
    /// provider: a tool returning null output caused the Python
    /// orchestrator to skip appending the ActionResult.
    #[test]
    fn json_to_thread_messages_every_tool_call_has_action_result() {
        // Simulate working_messages after the Python fix: every call gets
        // an ActionResult, even when the original output was null.
        let messages = serde_json::json!([
            {"role": "System", "content": "You are a helpful assistant."},
            {"role": "User", "content": "Update all tools."},
            {
                "role": "Assistant",
                "content": "",
                "action_calls": [
                    {"call_id": "call_AAA", "name": "tool_a", "params": {}},
                    {"call_id": "call_BBB", "name": "tool_b", "params": {}},
                    {"call_id": "call_CCC", "name": "tool_c", "params": {}}
                ]
            },
            {
                "role": "ActionResult",
                "content": "{\"ok\": true}",
                "action_name": "tool_a",
                "action_call_id": "call_AAA"
            },
            {
                "role": "ActionResult",
                "content": "[no output]",
                "action_name": "tool_b",
                "action_call_id": "call_BBB"
            },
            {
                "role": "ActionResult",
                "content": "{\"done\": true}",
                "action_name": "tool_c",
                "action_call_id": "call_CCC"
            }
        ]);

        let parsed = json_to_thread_messages(&messages).expect("must parse");
        assert_eq!(parsed.len(), 6);

        // Extract call IDs from the assistant message
        let assistant_calls: std::collections::HashSet<String> = parsed
            .iter()
            .filter_map(|m| m.action_calls.as_ref())
            .flat_map(|calls| calls.iter().map(|c| c.id.clone()))
            .collect();

        // Extract call IDs from ActionResult messages
        let result_call_ids: std::collections::HashSet<String> = parsed
            .iter()
            .filter(|m| m.role == crate::types::message::MessageRole::ActionResult)
            .filter_map(|m| m.action_call_id.clone())
            .collect();

        // Every tool_call must have a matching ActionResult
        for call_id in &assistant_calls {
            assert!(
                result_call_ids.contains(call_id),
                "tool_call {call_id} has no matching ActionResult — \
                 this would cause 'No tool output found' from the LLM API"
            );
        }
    }

    #[test]
    fn handle_emit_event_dispatches_budget_warning() {
        let mut thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            crate::types::project::ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        thread.transition_to(ThreadState::Running, None).unwrap();
        let args = vec![MontyObject::String("budget_warning".into())];
        let kwargs = vec![
            (
                MontyObject::String("field".into()),
                MontyObject::String("tokens".into()),
            ),
            (
                MontyObject::String("value".into()),
                MontyObject::Int(TEST_NEG_TOKENS_50),
            ),
            (
                MontyObject::String("message".into()),
                MontyObject::String("token budget low".into()),
            ),
        ];
        handle_emit_event(&args, &kwargs, &mut thread, None);
        let warned = thread.events.iter().any(|e| {
            matches!(
                &e.kind,
                EventKind::BudgetWarning { field, value, .. }
                    if field == "tokens" && *value == TEST_NEG_TOKENS_50
            )
        });
        assert!(warned, "expected BudgetWarning event on thread");
    }

    #[test]
    fn handle_emit_event_dispatches_prompt_over_budget() {
        let mut thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            crate::types::project::ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        thread.transition_to(ThreadState::Running, None).unwrap();
        let args = vec![MontyObject::String("prompt_over_budget".into())];
        let kwargs = vec![
            (
                MontyObject::String("estimated_tokens".into()),
                MontyObject::Int(TEST_ESTIMATED_TOKENS_8K),
            ),
            (
                MontyObject::String("budget_tokens".into()),
                MontyObject::Int(TEST_BUDGET_TOKENS_6K),
            ),
        ];
        handle_emit_event(&args, &kwargs, &mut thread, None);
        let over = thread.events.iter().any(|e| {
            matches!(
                &e.kind,
                EventKind::PromptOverBudget {
                    estimated_tokens,
                    budget_tokens,
                } if *estimated_tokens == TEST_ESTIMATED_TOKENS_8K as u64 && *budget_tokens == TEST_BUDGET_TOKENS_6K as u64
            )
        });
        assert!(over, "expected PromptOverBudget event on thread");
    }

    #[test]
    fn handle_emit_event_dispatches_recipe_tier_zero_events() {
        // v3 Phase H4.2: the three Tier-0 events emitted by the Model B/C
        // agent-loop Tier-0 path (the `LoopOrchestratorPort` driver + the
        // engine `pub` fns extracted in H.8) must be recorded as typed
        // `EventKind` variants on `thread.events` (before H4.2 the
        // fallthrough DROPPED them). The Model A `default.py` step-0
        // `tier_zero` branch that previously emitted these was removed in
        // v3 Phase H.5 O3; the kwarg shapes it established are preserved.
        // `recipe_id` rides the `recipe_id` kwarg, `recipe_name` rides the
        // `recipe` kwarg, and `message` (failed only) rides the `message`
        // kwarg — matching the H4.5 emit shapes.
        let mut thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            crate::types::project::ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        thread.transition_to(ThreadState::Running, None).unwrap();

        let recipe_id = "11111111-1111-1111-1111-111111111111";
        let recipe_name = "greet-recipe";

        // started
        let args = vec![MontyObject::String("recipe_tier_zero_started".into())];
        let kwargs = vec![
            (
                MontyObject::String("recipe".into()),
                MontyObject::String(recipe_name.into()),
            ),
            (
                MontyObject::String("recipe_id".into()),
                MontyObject::String(recipe_id.into()),
            ),
        ];
        handle_emit_event(&args, &kwargs, &mut thread, None);

        // succeeded
        let args = vec![MontyObject::String("recipe_tier_zero_succeeded".into())];
        handle_emit_event(&args, &kwargs, &mut thread, None);

        // failed (adds a `message` kwarg)
        let args = vec![MontyObject::String("recipe_tier_zero_failed".into())];
        let mut failed_kwargs = kwargs.clone();
        failed_kwargs.push((
            MontyObject::String("message".into()),
            MontyObject::String("step raised: boom".into()),
        ));
        handle_emit_event(&args, &failed_kwargs, &mut thread, None);

        let started = thread.events.iter().any(|e| {
            matches!(
                &e.kind,
                EventKind::RecipeTierZeroStarted {
                    recipe_id: id,
                    recipe_name: name
                } if id.as_str() == recipe_id && name == recipe_name
            )
        });
        assert!(started, "expected RecipeTierZeroStarted event on thread");

        let succeeded = thread.events.iter().any(|e| {
            matches!(
                &e.kind,
                EventKind::RecipeTierZeroSucceeded {
                    recipe_id: id,
                    recipe_name: name
                } if id.as_str() == recipe_id && name == recipe_name
            )
        });
        assert!(
            succeeded,
            "expected RecipeTierZeroSucceeded event on thread"
        );

        let failed = thread.events.iter().any(|e| {
            matches!(
                &e.kind,
                EventKind::RecipeTierZeroFailed {
                    recipe_id: id,
                    recipe_name: name,
                    message
                } if id.as_str() == recipe_id
                    && name == recipe_name
                    && message == "step raised: boom"
            )
        });
        assert!(failed, "expected RecipeTierZeroFailed event on thread");
    }

    #[test]
    fn build_tier_zero_outcome_success_via_extra_stamp() {
        // v3 Phase H4.6 branch (1): the success `extra` stamp present in
        // the result dict → Some(success == true). Mirrors the H4.5
        // success-path stamp shape
        // `complete_result(state, "completed", response=...,
        // extra={"tier_zero_outcome": {"recipe_id": ..., "success":
        // True}})`. The stamp is now written by the Model B/C agent-loop
        // Tier-0 path (the `LoopOrchestratorPort` driver + the engine
        // `pub` fns extracted in H.8); the Model A `default.py` writer
        // was removed in v3 Phase H.5 O3, but the stamp shape this reader
        // expects is preserved. A `RecipeTierZeroStarted` event alone
        // must NOT be misread as the terminal outcome (it is non-terminal).
        let thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            crate::types::project::ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        let result = serde_json::json!({
            "outcome": "completed",
            "response": "hello",
            "tier_zero_outcome": {
                "recipe_id": "11111111-1111-1111-1111-111111111111",
                "success": true
            }
        });
        let events = vec![ThreadEvent::new(
            thread.id,
            EventKind::RecipeTierZeroStarted {
                recipe_id: "11111111-1111-1111-1111-111111111111".to_string(),
                recipe_name: "greet-recipe".to_string(),
            },
        )];
        let outcome = build_tier_zero_outcome(&result, &events).expect("success stamp present");
        assert_eq!(outcome.recipe_id, "11111111-1111-1111-1111-111111111111");
        assert!(
            outcome.success,
            "the extra stamp presence is the success signal"
        );
    }

    #[test]
    fn build_tier_zero_outcome_failure_via_event() {
        // v3 Phase H4.6 branch (2): no success stamp (a Tier-0 failure
        // degrades to Tier-2; the result dict has NO `tier_zero_outcome`
        // stamp per Q-H6), but a `RecipeTierZeroFailed` event carrying a
        // recipe_id is on thread.events → Some(success == false).
        let thread = Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            crate::types::project::ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        let result = serde_json::json!({
            "outcome": "completed",
            "response": "done"
        });
        let events = vec![
            ThreadEvent::new(
                thread.id,
                EventKind::RecipeTierZeroStarted {
                    recipe_id: "22222222-2222-2222-2222-222222222222".to_string(),
                    recipe_name: "bad-recipe".to_string(),
                },
            ),
            ThreadEvent::new(
                thread.id,
                EventKind::RecipeTierZeroFailed {
                    recipe_id: "22222222-2222-2222-2222-222222222222".to_string(),
                    recipe_name: "bad-recipe".to_string(),
                    message: "step raised: boom".to_string(),
                },
            ),
        ];
        let outcome = build_tier_zero_outcome(&result, &events).expect("failed event present");
        assert_eq!(outcome.recipe_id, "22222222-2222-2222-2222-222222222222");
        assert!(
            !outcome.success,
            "a RecipeTierZeroFailed event is the failure signal"
        );
    }

    #[test]
    fn build_tier_zero_outcome_none_for_plain_tier2_turn() {
        // v3 Phase H4.6 branch (3): a plain Tier-2 LLM turn has no
        // Tier-0 stamp AND no Tier-0 events → None (no Tier-0 attempt).
        let result = serde_json::json!({
            "outcome": "completed",
            "response": "an llm reply"
        });
        let events: Vec<ThreadEvent> = Vec::new();
        assert!(
            build_tier_zero_outcome(&result, &events).is_none(),
            "a plain Tier-2 turn has no tier_zero_outcome"
        );
    }

    // ── Reduction-rules cache test serialization ───────────────────────────
    //
    // `invalidate_reduction_rules_cache()` is a process-wide flush: it clears
    // EVERY cached slot, not just the caller's. Four tests here call it and
    // `cargo test` runs them in parallel, so one test's invalidate can clear a
    // sibling test's slot between its two `load_reduction_rules` calls — turning
    // a "DB called exactly once" cache assertion into a spurious second query
    // (the intermittent `load_reduction_rules_db_error_returns_empty_and_caches`
    // failure). This test-only `std::sync::Mutex` serializes just the
    // reduction-rules cache tests so the process-wide flush cannot race with a
    // sibling's slot. The async tests hold the guard across `.await` on a
    // current-thread `#[tokio::test]` runtime (no thread migration → no
    // self-deadlock); see the `#[allow(clippy::await_holding_lock)]` on each,
    // matching the oauth/hooks convention.
    static REDUCTION_RULES_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn invalidate_reduction_rules_cache_returns_zero_when_unused() {
        let _test_lock = REDUCTION_RULES_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Ensure the public API exists and is callable; in this fresh
        // state no slots are populated so cleared is zero.
        let _ = invalidate_reduction_rules_cache();
    }

    // ── load_reduction_rules ──────────────────────────────────────────────

    /// Helper: build a `MemoryDoc` tagged `reduction_rule` whose content is
    /// a JSON array of rule objects.
    fn make_rule_doc(
        project_id: crate::types::project::ProjectId,
        user_id: &str,
        rules_json: serde_json::Value,
    ) -> crate::types::memory::MemoryDoc {
        use crate::types::memory::{DocType, MemoryDoc};
        let mut doc = MemoryDoc::new(
            project_id,
            user_id,
            DocType::Note,
            "rules",
            rules_json.to_string(),
        );
        doc.tags.push("reduction_rule".to_string());
        doc
    }

    // Lock held across await to serialize the reduction-rules cache tests
    // (process-wide invalidate race — see REDUCTION_RULES_TEST_LOCK).
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn load_reduction_rules_cache_miss_then_hit() {
        let _test_lock = REDUCTION_RULES_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // First call loads from DB; second call returns from cache (exactly
        // one DB query total, proven by using a store that counts calls).
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingStore {
            inner: crate::tests::InMemoryStore,
            calls: AtomicUsize,
        }

        #[async_trait::async_trait]
        impl Store for CountingStore {
            async fn list_memory_docs(
                &self,
                project_id: crate::types::project::ProjectId,
                user_id: &str,
            ) -> Result<Vec<crate::types::memory::MemoryDoc>, crate::types::error::EngineError>
            {
                self.calls.fetch_add(1, Ordering::Relaxed);
                self.inner.list_memory_docs(project_id, user_id).await
            }
            // ── delegate everything else ──────────────────────────────────
            async fn save_thread(
                &self,
                t: &crate::types::thread::Thread,
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.save_thread(t).await
            }
            async fn load_thread(
                &self,
                id: crate::types::thread::ThreadId,
            ) -> Result<Option<crate::types::thread::Thread>, crate::types::error::EngineError>
            {
                self.inner.load_thread(id).await
            }
            async fn list_threads(
                &self,
                p: crate::types::project::ProjectId,
                u: &str,
            ) -> Result<Vec<crate::types::thread::Thread>, crate::types::error::EngineError>
            {
                self.inner.list_threads(p, u).await
            }
            async fn update_thread_state(
                &self,
                id: crate::types::thread::ThreadId,
                s: crate::types::thread::ThreadState,
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.update_thread_state(id, s).await
            }
            async fn save_step(
                &self,
                step: &crate::types::step::Step,
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.save_step(step).await
            }
            async fn load_steps(
                &self,
                id: crate::types::thread::ThreadId,
            ) -> Result<Vec<crate::types::step::Step>, crate::types::error::EngineError>
            {
                self.inner.load_steps(id).await
            }
            async fn append_events(
                &self,
                evts: &[crate::types::event::ThreadEvent],
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.append_events(evts).await
            }
            async fn load_events(
                &self,
                id: crate::types::thread::ThreadId,
            ) -> Result<Vec<crate::types::event::ThreadEvent>, crate::types::error::EngineError>
            {
                self.inner.load_events(id).await
            }
            async fn save_project(
                &self,
                p: &crate::types::project::Project,
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.save_project(p).await
            }
            async fn load_project(
                &self,
                id: crate::types::project::ProjectId,
            ) -> Result<Option<crate::types::project::Project>, crate::types::error::EngineError>
            {
                self.inner.load_project(id).await
            }
            async fn list_all_projects(
                &self,
            ) -> Result<Vec<crate::types::project::Project>, crate::types::error::EngineError>
            {
                self.inner.list_all_projects().await
            }
            async fn save_conversation(
                &self,
                c: &crate::types::conversation::ConversationSurface,
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.save_conversation(c).await
            }
            async fn load_conversation(
                &self,
                id: crate::types::conversation::ConversationId,
            ) -> Result<
                Option<crate::types::conversation::ConversationSurface>,
                crate::types::error::EngineError,
            > {
                self.inner.load_conversation(id).await
            }
            async fn list_conversations(
                &self,
                u: &str,
            ) -> Result<
                Vec<crate::types::conversation::ConversationSurface>,
                crate::types::error::EngineError,
            > {
                self.inner.list_conversations(u).await
            }
            async fn save_memory_doc(
                &self,
                doc: &crate::types::memory::MemoryDoc,
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.save_memory_doc(doc).await
            }
            async fn load_memory_doc(
                &self,
                id: crate::types::memory::DocId,
            ) -> Result<Option<crate::types::memory::MemoryDoc>, crate::types::error::EngineError>
            {
                self.inner.load_memory_doc(id).await
            }
            async fn list_memory_docs_by_owner(
                &self,
                u: &str,
            ) -> Result<Vec<crate::types::memory::MemoryDoc>, crate::types::error::EngineError>
            {
                self.inner.list_memory_docs_by_owner(u).await
            }
            async fn save_lease(
                &self,
                l: &crate::types::capability::CapabilityLease,
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.save_lease(l).await
            }
            async fn load_active_leases(
                &self,
                id: crate::types::thread::ThreadId,
            ) -> Result<
                Vec<crate::types::capability::CapabilityLease>,
                crate::types::error::EngineError,
            > {
                self.inner.load_active_leases(id).await
            }
            async fn revoke_lease(
                &self,
                id: crate::types::capability::LeaseId,
                reason: &str,
            ) -> Result<(), crate::types::error::EngineError> {
                self.inner.revoke_lease(id, reason).await
            }
        }

        let project_id = crate::types::project::ProjectId::new();
        let user_id = "alice";
        let rule = serde_json::json!([{"type": "drop", "field": "content"}]);
        let doc = make_rule_doc(project_id, user_id, rule.clone());

        // Wrap in Arc so we can also hold a reference to the CountingStore
        // and verify call counts directly (no as_any needed).
        let counting_store = Arc::new(CountingStore {
            inner: crate::tests::InMemoryStore::with_docs(vec![doc]),
            calls: AtomicUsize::new(0),
        });
        let store: Arc<dyn Store> = counting_store.clone();

        // Ensure a clean cache slot for this key before the test runs.
        invalidate_reduction_rules_cache();

        // First call: cache miss → DB query.
        let rules = load_reduction_rules(project_id, user_id, Some(&store)).await;
        assert_eq!(rules.len(), 1, "expected one rule from DB");
        assert_eq!(rules[0]["type"], "drop");
        assert_eq!(
            counting_store.calls.load(Ordering::Relaxed),
            1,
            "DB must be called exactly once on cache miss"
        );

        // Second call: cache hit → no additional DB query.
        let rules2 = load_reduction_rules(project_id, user_id, Some(&store)).await;
        assert_eq!(rules2.len(), 1, "expected same rule from cache");
        assert_eq!(
            counting_store.calls.load(Ordering::Relaxed),
            1,
            "DB must not be called again on cache hit"
        );
        assert_eq!(rules, rules2, "cache hit must return identical data");
    }

    // Lock held across await to serialize the reduction-rules cache tests
    // (process-wide invalidate race — see REDUCTION_RULES_TEST_LOCK).
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn load_reduction_rules_ignores_docs_without_tag() {
        let _test_lock = REDUCTION_RULES_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // A doc without the `reduction_rule` tag must be silently skipped.
        use crate::types::memory::{DocType, MemoryDoc};
        let project_id = crate::types::project::ProjectId::new();
        let user_id = "bob";
        let mut doc = MemoryDoc::new(
            project_id,
            user_id,
            DocType::Note,
            "not-a-rule",
            r#"[{"type": "drop", "field": "content"}]"#,
        );
        doc.tags.push("skill".to_string()); // wrong tag — must be ignored

        invalidate_reduction_rules_cache();

        let store: Arc<dyn Store> = Arc::new(crate::tests::InMemoryStore::with_docs(vec![doc]));
        let rules = load_reduction_rules(project_id, user_id, Some(&store)).await;
        assert!(
            rules.is_empty(),
            "docs without the 'reduction_rule' tag must not produce rules"
        );
    }

    // Lock held across await to serialize the reduction-rules cache tests
    // (process-wide invalidate race — see REDUCTION_RULES_TEST_LOCK).
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn load_reduction_rules_db_error_returns_empty_and_caches() {
        let _test_lock = REDUCTION_RULES_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // When the DB fails the function must return an empty vec and cache
        // that result so the slot is not re-queried on every subsequent call.
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct AlwaysFailStore {
            calls: AtomicUsize,
        }

        #[async_trait::async_trait]
        impl Store for AlwaysFailStore {
            async fn list_memory_docs(
                &self,
                _project_id: crate::types::project::ProjectId,
                _user_id: &str,
            ) -> Result<Vec<crate::types::memory::MemoryDoc>, crate::types::error::EngineError>
            {
                self.calls.fetch_add(1, Ordering::Relaxed);
                Err(crate::types::error::EngineError::Store {
                    reason: "simulated DB failure".into(),
                })
            }
            async fn save_thread(
                &self,
                _: &crate::types::thread::Thread,
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
            async fn load_thread(
                &self,
                _: crate::types::thread::ThreadId,
            ) -> Result<Option<crate::types::thread::Thread>, crate::types::error::EngineError>
            {
                Ok(None)
            }
            async fn list_threads(
                &self,
                _: crate::types::project::ProjectId,
                _: &str,
            ) -> Result<Vec<crate::types::thread::Thread>, crate::types::error::EngineError>
            {
                Ok(vec![])
            }
            async fn update_thread_state(
                &self,
                _: crate::types::thread::ThreadId,
                _: crate::types::thread::ThreadState,
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
            async fn save_step(
                &self,
                _: &crate::types::step::Step,
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
            async fn load_steps(
                &self,
                _: crate::types::thread::ThreadId,
            ) -> Result<Vec<crate::types::step::Step>, crate::types::error::EngineError>
            {
                Ok(vec![])
            }
            async fn append_events(
                &self,
                _: &[crate::types::event::ThreadEvent],
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
            async fn load_events(
                &self,
                _: crate::types::thread::ThreadId,
            ) -> Result<Vec<crate::types::event::ThreadEvent>, crate::types::error::EngineError>
            {
                Ok(vec![])
            }
            async fn save_project(
                &self,
                _: &crate::types::project::Project,
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
            async fn load_project(
                &self,
                _: crate::types::project::ProjectId,
            ) -> Result<Option<crate::types::project::Project>, crate::types::error::EngineError>
            {
                Ok(None)
            }
            async fn list_all_projects(
                &self,
            ) -> Result<Vec<crate::types::project::Project>, crate::types::error::EngineError>
            {
                Ok(vec![])
            }
            async fn save_conversation(
                &self,
                _: &crate::types::conversation::ConversationSurface,
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
            async fn load_conversation(
                &self,
                _: crate::types::conversation::ConversationId,
            ) -> Result<
                Option<crate::types::conversation::ConversationSurface>,
                crate::types::error::EngineError,
            > {
                Ok(None)
            }
            async fn list_conversations(
                &self,
                _: &str,
            ) -> Result<
                Vec<crate::types::conversation::ConversationSurface>,
                crate::types::error::EngineError,
            > {
                Ok(vec![])
            }
            async fn save_memory_doc(
                &self,
                _: &crate::types::memory::MemoryDoc,
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
            async fn load_memory_doc(
                &self,
                _: crate::types::memory::DocId,
            ) -> Result<Option<crate::types::memory::MemoryDoc>, crate::types::error::EngineError>
            {
                Ok(None)
            }
            async fn list_memory_docs_by_owner(
                &self,
                _: &str,
            ) -> Result<Vec<crate::types::memory::MemoryDoc>, crate::types::error::EngineError>
            {
                Ok(vec![])
            }
            async fn save_lease(
                &self,
                _: &crate::types::capability::CapabilityLease,
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
            async fn load_active_leases(
                &self,
                _: crate::types::thread::ThreadId,
            ) -> Result<
                Vec<crate::types::capability::CapabilityLease>,
                crate::types::error::EngineError,
            > {
                Ok(vec![])
            }
            async fn revoke_lease(
                &self,
                _: crate::types::capability::LeaseId,
                _: &str,
            ) -> Result<(), crate::types::error::EngineError> {
                Ok(())
            }
        }

        let project_id = crate::types::project::ProjectId::new();
        let user_id = "carol";

        invalidate_reduction_rules_cache();

        let store = Arc::new(AlwaysFailStore {
            calls: AtomicUsize::new(0),
        });
        let store_dyn: Arc<dyn Store> = store.clone();

        let rules = load_reduction_rules(project_id, user_id, Some(&store_dyn)).await;
        assert!(rules.is_empty(), "DB error must yield empty rules");
        assert_eq!(
            store.calls.load(Ordering::Relaxed),
            1,
            "DB called exactly once"
        );

        // Second call must use the cached empty vec (DB error is cached).
        let rules2 = load_reduction_rules(project_id, user_id, Some(&store_dyn)).await;
        assert!(rules2.is_empty());
        assert_eq!(
            store.calls.load(Ordering::Relaxed),
            1,
            "DB must not be called again after error was cached"
        );
    }

    // ── handle_log_budget_warning ─────────────────────────────────────────

    #[test]
    fn log_budget_warning_positional_args() {
        // The Python call site in default.py uses positional args:
        //   __log_budget_warning__("tokens", value, "token budget low")
        // This test verifies the positional path is wired correctly so a
        // future arg-order change in the Rust handler does not silently
        // produce garbled BudgetWarning events (the kwargs test in
        // handle_emit_event_dispatches_budget_warning only covers kwargs).
        let mut thread = crate::types::thread::Thread::new(
            "goal",
            crate::types::thread::ThreadType::Foreground,
            crate::types::project::ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        thread
            .transition_to(crate::types::thread::ThreadState::Running, None)
            .unwrap();

        let args = vec![
            MontyObject::String("tokens".into()),
            MontyObject::Int(TEST_NEG_TOKENS_42),
            MontyObject::String("token budget low".into()),
        ];
        let kwargs: Vec<(MontyObject, MontyObject)> = vec![];
        handle_log_budget_warning(&args, &kwargs, &mut thread, None);

        let warned = thread.events.iter().any(|e| {
            matches!(
                &e.kind,
                EventKind::BudgetWarning { field, value, message }
                    if field == "tokens" && *value == TEST_NEG_TOKENS_42 && message == "token budget low"
            )
        });
        assert!(
            warned,
            "positional-args path of handle_log_budget_warning must emit a BudgetWarning event \
             with the correct field, value, and message"
        );
    }

    // ── Sub-step 3.4: __validate_component__ reroute tests ──────────────────
    //
    // Verify that handle_validate_component creates an update-candidate in Q1,
    // that protected components get the 05:validator tag + llm_audit_required,
    // and that empty payloads / missing stores are no-ops.

    fn make_validate_thread() -> Thread {
        Thread::new(
            "test goal",
            crate::types::thread::ThreadType::Foreground,
            ProjectId::new(),
            "test-user",
            crate::types::thread::ThreadConfig::default(),
        )
    }

    #[tokio::test]
    async fn validate_component_queues_update_candidate_in_q1() {
        let store: Arc<dyn Store> = Arc::new(crate::tests::InMemoryStore::with_docs(vec![]));
        let thread = make_validate_thread();
        let args = vec![
            MontyObject::String("my-skill".into()),
            MontyObject::String("skill content here".into()),
            MontyObject::String("skill".into()),
        ];

        let result = handle_validate_component(&args, &thread, Some(&store)).await;

        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["queued"], serde_json::json!(true));
        assert_eq!(json["validation_status"], serde_json::json!("pending"));
        assert_eq!(json["queue_code"], serde_json::json!("q1_auto"));

        let docs = store
            .list_memory_docs(thread.project_id, &thread.user_id)
            .await
            .unwrap();
        assert_eq!(
            docs.len(),
            1,
            "exactly one update-candidate must be written"
        );
    }

    #[tokio::test]
    async fn validate_component_no_op_on_empty_payload() {
        let store: Arc<dyn Store> = Arc::new(crate::tests::InMemoryStore::with_docs(vec![]));
        let thread = make_validate_thread();
        let args = vec![
            MontyObject::String("my-skill".into()),
            MontyObject::String("".into()),
        ];

        let result = handle_validate_component(&args, &thread, Some(&store)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["queued"], serde_json::json!(false));

        let docs = store
            .list_memory_docs(thread.project_id, &thread.user_id)
            .await
            .unwrap();
        assert!(docs.is_empty(), "no doc must be written for empty payload");
    }

    #[tokio::test]
    async fn validate_component_no_op_without_store() {
        let thread = make_validate_thread();
        let args = vec![
            MontyObject::String("my-skill".into()),
            MontyObject::String("content".into()),
        ];

        let result = handle_validate_component(&args, &thread, None).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["queued"], serde_json::json!(false));
        assert_eq!(json["reason"], serde_json::json!("no_store"));
    }

    #[tokio::test]
    async fn validate_component_protected_title_sets_validator_tag_and_audit_flag() {
        let store: Arc<dyn Store> = Arc::new(crate::tests::InMemoryStore::with_docs(vec![]));
        let thread = make_validate_thread();
        // "orchestrator:main" is a protected title (class 10 / Orchestrator)
        assert!(
            crate::executor::prompt::is_protected_component_title("orchestrator:main"),
            "orchestrator:main must be a protected component"
        );
        let args = vec![
            MontyObject::String("orchestrator:main".into()),
            MontyObject::String("def run_loop(): pass".into()),
            MontyObject::String("skill".into()),
        ];

        let result = handle_validate_component(&args, &thread, Some(&store)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["queued"], serde_json::json!(true));
        assert_eq!(json["llm_audit_required"], serde_json::json!(true));
        assert_eq!(json["llm_audit_status"], serde_json::json!("pending"));

        let docs = store
            .list_memory_docs(thread.project_id, &thread.user_id)
            .await
            .unwrap();
        assert_eq!(docs.len(), 1);
        let tags = &docs[0].tags;
        assert!(
            tags.iter().any(|t| t == "05:validator"),
            "protected component candidate must carry 05:validator tag, got: {tags:?}"
        );
    }

    #[tokio::test]
    async fn validate_component_non_protected_title_skips_audit_flag() {
        let store: Arc<dyn Store> = Arc::new(crate::tests::InMemoryStore::with_docs(vec![]));
        let thread = make_validate_thread();
        let args = vec![
            MontyObject::String("my-custom-skill".into()),
            MontyObject::String("skill content here".into()),
            MontyObject::String("skill".into()),
        ];

        let result = handle_validate_component(&args, &thread, Some(&store)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };
        assert_eq!(json["queued"], serde_json::json!(true));
        assert_eq!(json["llm_audit_required"], serde_json::json!(false));
        assert_eq!(json["llm_audit_status"], serde_json::json!("not_required"));
    }

    // ── Phase F.7: RetrievalSource arm tests ────────────────────────────────
    //
    // The plan's Phase F test list (saved_plan_to_v3.md:5190–5198). These drive
    // `assemble_prior_knowledge_with_hint` (the H8.2 pub fn that replaced the
    // dormant Model A `handle_assemble_prior_knowledge` handler) on the
    // None-branch (fresh `fetch_for_turn`) through a mock `RetrievalSource` so
    // the `PkrAssemblyResult` fields + prose `orchestrator_content` (FINDING F)
    // are verified without a live Postgres. Tests #8/#9 (DB-integration) live
    // in `crates/brassclaw_reborn_composition/tests/fetch_component.rs`.

    /// In-memory `RetrievalSource` for Phase F.7 tests. Captures the
    /// `ComponentScope` handed to `fetch_for_turn` (test #7) and returns a
    /// preset `FetchForTurnResult` on the first call (tests #1–#5). Subsequent
    /// calls return an empty `Components` result.
    struct MockRetrievalSource {
        result: Mutex<Option<FetchForTurnResult>>,
        captured_scope: Arc<Mutex<Option<ComponentScope>>>,
    }

    impl MockRetrievalSource {
        fn new(
            result: FetchForTurnResult,
            captured_scope: Arc<Mutex<Option<ComponentScope>>>,
        ) -> Self {
            Self {
                result: Mutex::new(Some(result)),
                captured_scope,
            }
        }
    }

    #[async_trait]
    impl RetrievalSource for MockRetrievalSource {
        async fn fetch_for_consumer(
            &self,
            _scope: &ComponentScope,
            _query: &str,
            _token_budget: usize,
            _consumer_tag: &str,
        ) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
            Ok(Vec::new())
        }

        async fn fetch_for_turn(
            &self,
            scope: &ComponentScope,
            _query: &str,
            _token_budget: usize,
            _sender_class_code: &str,
        ) -> Result<FetchForTurnResult, RetrievalSourceError> {
            *self.captured_scope.lock().unwrap() = Some(scope.clone());
            let result = self
                .result
                .lock()
                .unwrap()
                .take()
                .unwrap_or(FetchForTurnResult::Components(Vec::new()));
            Ok(result)
        }
    }

    /// Build a `ComponentItem` with a deterministic UUID (u128 seed).
    fn phase_f7_item(seed: u128, class_code: i32, name: &str, content: &str) -> ComponentItem {
        ComponentItem {
            id: uuid::Uuid::from_u128(seed),
            class_code,
            prompt_uid: seed as i64,
            name: name.to_string(),
            description: String::new(),
            effective_content: content.to_string(),
            override_prompt_creation: false,
            // Test fixture: no executable Action steps (Q-G-STUB1).
            steps: None,
            allowed_tools: None,
        }
    }

    /// Build a foreground `Thread` for `assemble_prior_knowledge_with_hint`.
    fn phase_f7_thread(goal: &str) -> Thread {
        crate::types::thread::Thread::new(
            goal,
            crate::types::thread::ThreadType::Foreground,
            ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        )
    }

    /// Call `assemble_prior_knowledge_with_hint` (the H8.2 pub fn that replaced
    /// the dormant Model A `handle_assemble_prior_knowledge` Monty handler) with
    /// a `RetrievalSource` and NO `recipe_hint` (None-branch → fresh
    /// `fetch_for_turn`), returning the [`PkrAssemblyResult`].
    async fn phase_f7_assemble(
        src: Arc<dyn RetrievalSource>,
        thread: &Thread,
    ) -> PkrAssemblyResult {
        assemble_prior_knowledge_with_hint(
            thread,
            &thread.goal,
            TEST_TOKEN_ALLOC_2K as usize,
            "02",
            Some(&src),
            None,
        )
        .await
        .expect("assemble_prior_knowledge_with_hint must succeed in unit tests")
    }

    /// A `SplitResult` with a Skill + PythonCode in the orchestrator channel
    /// and a ToolSkill in the Rust channel (tests #1 / #2).
    fn phase_f7_split_result() -> FetchForTurnResult {
        let skill = phase_f7_item(1, 3, "ls", "list files in a directory");
        let pycode = phase_f7_item(2, 22, "ls-result-handler", "print(result)");
        let toolskill = phase_f7_item(3, 13, "ls-tool", "secret rust tool body");
        let routing = TurnRoutingSignals {
            override_prompt_creation: false,
            matched_component_ids: vec![skill.id.to_string(), pycode.id.to_string()],
            variant_label: "default".to_string(),
            step_link: "recipe_ls#step1".to_string(),
            llm_call_required: true,
            wilson_lower: 0.0,
            tier0_eligible: false,
            recipe_id: None,
            recipe_name: "recipe_ls".to_string(),
        };
        FetchForTurnResult::SplitResult {
            rust_items: vec![toolskill],
            orchestrator_items: vec![skill, pycode],
            routing,
            instruction: None,
        }
    }

    /// Phase F.7 #1 — `SplitResult` `orchestrator_content` is the prose
    /// StepContextSpec-headed block: it carries the Skill + PythonCode bodies
    /// with Capitalized headings, does NOT carry the Rust-channel ToolSkill
    /// body, and contains no `type:text` / Annotation step info.
    #[tokio::test]
    async fn phase_f7_split_result_orchestrator_content_is_prose_with_skill_and_pythoncode() {
        let src: Arc<dyn RetrievalSource> = Arc::new(MockRetrievalSource::new(
            phase_f7_split_result(),
            Arc::new(Mutex::new(None)),
        ));
        let thread = phase_f7_thread("list files");
        let result = phase_f7_assemble(src, &thread).await;

        let oc = &result.orchestrator_content;

        // Skill + PythonCode bodies present with Capitalized headings.
        assert!(oc.contains("## [Skill: ls]"), "missing Skill heading: {oc}");
        assert!(
            oc.contains("list files in a directory"),
            "missing skill body: {oc}"
        );
        assert!(
            oc.contains("## [PythonCode: ls-result-handler]"),
            "missing PythonCode heading: {oc}"
        );
        assert!(
            oc.contains("print(result)"),
            "missing pythoncode body: {oc}"
        );

        // ToolSkill (class 13) is Rust-channel-only: never in orchestrator_content.
        assert!(
            !oc.contains("secret rust tool body"),
            "ToolSkill body must NOT appear in orchestrator_content: {oc}"
        );
        assert!(
            !oc.contains("## [ToolSkill:"),
            "no ToolSkill heading must be emitted: {oc}"
        );

        // No type:"text" / Annotation step info in orchestrator_content.
        assert!(
            !oc.contains("type:text"),
            "no type:text step info in orchestrator_content: {oc}"
        );
        assert!(
            !oc.contains("## [Annotation:"),
            "no Annotation heading from a ComponentItem: {oc}"
        );
    }

    /// Phase F.7 #2 — `SplitResult` `orchestrator_content` is the prose
    /// StepContextSpec-headed block (FINDING F: was a JSON object, now a prose
    /// string). The reduced `PkrAssemblyResult` (Q2) no longer carries a
    /// separate `formatted_content` alias, so this test asserts the invariant
    /// #1 does not: `orchestrator_content` is prose (starts with `## [`) and
    /// is NOT a JSON object (does not start with `{`).
    #[tokio::test]
    async fn phase_f7_split_result_orchestrator_content_is_prose_not_json() {
        let src: Arc<dyn RetrievalSource> = Arc::new(MockRetrievalSource::new(
            phase_f7_split_result(),
            Arc::new(Mutex::new(None)),
        ));
        let thread = phase_f7_thread("list files");
        let result = phase_f7_assemble(src, &thread).await;

        let oc = &result.orchestrator_content;
        assert!(
            oc.starts_with("## ["),
            "orchestrator_content must be a prose StepContextSpec block, got: {oc}"
        );
        assert!(
            !oc.starts_with('{'),
            "orchestrator_content must NOT be a JSON object (FINDING F), got: {oc}"
        );
    }

    /// Phase F.7 #3 — `ActionShortCircuit` sets `action_short_circuit: true`
    /// and emits an empty `orchestrator_content` (an Action executes directly,
    /// no LLM prior knowledge).
    #[tokio::test]
    async fn phase_f7_action_short_circuit_emits_empty_orchestrator_content() {
        let component_id = uuid::Uuid::from_u128(42);
        let fetch = FetchForTurnResult::ActionShortCircuit {
            component_id,
            name: "deploy".to_string(),
        };
        let src: Arc<dyn RetrievalSource> =
            Arc::new(MockRetrievalSource::new(fetch, Arc::new(Mutex::new(None))));
        let thread = phase_f7_thread("deploy now");
        let result = phase_f7_assemble(src, &thread).await;

        assert!(
            result.action_short_circuit,
            "ActionShortCircuit must set action_short_circuit"
        );
        assert_eq!(
            result.orchestrator_content, "",
            "ActionShortCircuit has no prior knowledge -> empty orchestrator_content"
        );
        let cid = component_id.to_string();
        assert_eq!(
            result.action_component_id.as_deref(),
            Some(cid.as_str()),
            "ActionShortCircuit carries the action component id"
        );
        assert_eq!(result.action_name.as_deref(), Some("deploy"));
        assert_eq!(result.matched_component_ids, vec![cid]);
    }

    /// Phase F.7 #4 — `Components` (no-match broad scan) `orchestrator_content`
    /// contains every emittable retrieved item (Q-F7-1) with Capitalized
    /// headings, skips class-13 ToolSkill (§0.9 invariant), yet keeps all item
    /// ids in `matched_component_ids`. Not a short-circuit / disambiguation.
    #[tokio::test]
    async fn phase_f7_components_arm_orchestrator_content_contains_all_emittable_items() {
        let skill = phase_f7_item(10, 3, "grep", "search file contents");
        let spec = phase_f7_item(11, 12, "search-spec", "spec body text");
        let action = phase_f7_item(12, 16, "search-action", "action body text");
        let toolskill = phase_f7_item(13, 13, "search-tool", "hidden tool body");
        let fetch = FetchForTurnResult::Components(vec![skill, spec, action, toolskill]);

        let src: Arc<dyn RetrievalSource> =
            Arc::new(MockRetrievalSource::new(fetch, Arc::new(Mutex::new(None))));
        let thread = phase_f7_thread("search files");
        let result = phase_f7_assemble(src, &thread).await;

        let oc = &result.orchestrator_content;

        // All emittable classes labelled with Capitalized headings + bodies.
        assert!(oc.contains("## [Skill: grep]"), "{oc}");
        assert!(oc.contains("search file contents"), "{oc}");
        assert!(oc.contains("## [Spec: search-spec]"), "{oc}");
        assert!(oc.contains("spec body text"), "{oc}");
        assert!(oc.contains("## [Action: search-action]"), "{oc}");
        assert!(oc.contains("action body text"), "{oc}");

        // Class 13 ToolSkill skipped from orchestrator_content (§0.9 invariant)...
        assert!(!oc.contains("hidden tool body"), "{oc}");
        assert!(!oc.contains("## [ToolSkill:"), "{oc}");

        // ...but its id is still in matched_component_ids (all items).
        assert_eq!(
            result.matched_component_ids.len(),
            4,
            "all 4 item ids in matched_component_ids"
        );

        // Not a short-circuit / disambiguation.
        assert!(!result.action_short_circuit);
        assert!(!result.disambiguation);
        assert!(!result.override_prompt_creation);
    }

    /// Phase F.7 #5 — `Disambiguation` sets `disambiguation: true` and surfaces
    /// the candidate list (component_id / class_code / class_label / score).
    #[tokio::test]
    async fn phase_f7_disambiguation_surfaces_candidates() {
        let candidates = vec![
            IntentCandidate {
                row_id: uuid::Uuid::from_u128(100),
                component_id: uuid::Uuid::from_u128(101),
                component_class_code: 3,
                input_class: 0,
                score: 5,
                class_label: "skill_rusty".to_string(),
            },
            IntentCandidate {
                row_id: uuid::Uuid::from_u128(102),
                component_id: uuid::Uuid::from_u128(103),
                component_class_code: 21,
                input_class: 0,
                score: 5,
                class_label: "recipe".to_string(),
            },
        ];
        let fetch = FetchForTurnResult::Disambiguation(candidates);
        let src: Arc<dyn RetrievalSource> =
            Arc::new(MockRetrievalSource::new(fetch, Arc::new(Mutex::new(None))));
        let thread = phase_f7_thread("ambiguous query");
        let result = phase_f7_assemble(src, &thread).await;

        assert!(result.disambiguation);
        assert_eq!(result.orchestrator_content, "");

        let arr = &result.candidates;
        assert_eq!(arr.len(), 2, "two disambiguation candidates");
        assert_eq!(arr[0]["component_class_code"], 3);
        assert_eq!(arr[0]["class_label"], "skill_rusty");
        assert_eq!(arr[0]["score"], 5);
        assert_eq!(
            arr[0]["component_id"],
            uuid::Uuid::from_u128(101).to_string()
        );
        assert_eq!(arr[1]["component_class_code"], 21);
        assert_eq!(arr[1]["class_label"], "recipe");
    }

    /// Phase G.2 — `handle_resolve_component_by_name` returns `Value::Null`
    /// whenever the SEC-01-validated named lookup cannot run: no component port
    /// (`None` port — non-skills-db config / unit-test path), an empty name, or
    /// a missing class-code arg. The skills-db-populated (validated component
    /// found) case is a DB-integration test (composition `tests/`,
    /// skip-if-no-docker) mirroring `fetch_component.rs`.
    #[tokio::test]
    async fn phase_g2_resolve_by_name_returns_null_on_unresolvable_paths() {
        let thread = phase_f7_thread("deploy now");

        async fn null_from(args: Vec<MontyObject>, thread: &Thread) -> serde_json::Value {
            let result = handle_resolve_component_by_name(&args, thread, None).await;
            match result {
                ExtFunctionResult::Return(obj) => monty_to_json(&obj),
                other => panic!("expected Return, got: {other:?}"),
            }
        }

        // No-pool path (the unit-test reality): a well-formed call returns Null.
        let json = null_from(
            vec![MontyObject::String("deploy".into()), MontyObject::Int(16)],
            &thread,
        )
        .await;
        assert!(json.is_null(), "no-pool path must return Null, got {json}");

        // Empty name early-returns Null.
        let json = null_from(
            vec![MontyObject::String("".into()), MontyObject::Int(16)],
            &thread,
        )
        .await;
        assert!(json.is_null(), "empty name must return Null, got {json}");

        // Missing class-code arg returns Null.
        let json = null_from(vec![MontyObject::String("deploy".into())], &thread).await;
        assert!(
            json.is_null(),
            "missing class-code must return Null, got {json}"
        );
    }

    /// Phase F.7 #6 — `__retrieve_docs__` is untouched by Phase F: it still
    /// returns a FLAT list of `{type, title, content}` entries, not a dict
    /// with the v3 routing keys.
    #[tokio::test]
    async fn phase_f7_handle_retrieve_docs_remains_flat_list() {
        let project = ProjectId::new();
        let retrieval =
            RetrievalEngine::new(Arc::new(crate::tests::InMemoryStore::with_docs(vec![
                MemoryDoc::new(
                    project,
                    "user",
                    DocType::Lesson,
                    "web_search tool alias",
                    "Use web_search",
                ),
            ])));
        let thread = crate::types::thread::Thread::new(
            "web_search error",
            crate::types::thread::ThreadType::Foreground,
            project,
            "user",
            crate::types::thread::ThreadConfig::default(),
        );
        let args = vec![
            MontyObject::String("web_search error".into()),
            MontyObject::Int(5),
        ];

        let result = handle_retrieve_docs(&args, &[], &thread, Some(&retrieval)).await;
        let json = match result {
            ExtFunctionResult::Return(obj) => monty_to_json(&obj),
            other => panic!("expected Return, got: {other:?}"),
        };

        // Phase F must leave __retrieve_docs__ returning a FLAT array of
        // {type, title, content} entries — NOT a dict with v3 routing keys.
        let arr = json
            .as_array()
            .expect("retrieve_docs must return a flat array");
        assert!(!arr.is_empty(), "the seeded lesson must be retrieved");
        let first = &arr[0];
        assert!(first.get("type").is_some(), "type key required");
        assert!(first.get("title").is_some(), "title key required");
        assert!(first.get("content").is_some(), "content key required");
        assert_eq!(first["title"], "web_search tool alias");
        assert_eq!(first["content"], "Use web_search");

        // No v3 routing / orchestrator keys leaked into the flat list entries.
        assert!(first.get("orchestrator_content").is_none());
        assert!(first.get("matched_component_ids").is_none());
        assert!(first.get("action_short_circuit").is_none());
        assert!(first.get("disambiguation").is_none());
    }

    /// Phase F.7 #7 — the `ComponentScope` built by
    /// `assemble_prior_knowledge_with_hint` (the H8.2 pub fn that replaced the
    /// dormant Model A `handle_assemble_prior_knowledge` handler) carries the
    /// thread's `tenant_id` + `agent_id` (the F.1 / F.3 fix), not the old
    /// `user_id` / `"default"` stub.
    #[tokio::test]
    async fn phase_f7_assemble_scope_uses_thread_tenant_and_agent() {
        let captured: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
        let result = FetchForTurnResult::Components(Vec::new());
        let src: Arc<dyn RetrievalSource> =
            Arc::new(MockRetrievalSource::new(result, captured.clone()));
        let thread = crate::types::thread::Thread::new(
            "scoped goal",
            crate::types::thread::ThreadType::Foreground,
            ProjectId::new(),
            "user",
            crate::types::thread::ThreadConfig::default(),
        )
        .with_tenant_agent("tenant-t", "agent-a");

        let _ = phase_f7_assemble(src, &thread).await;

        let scope = captured
            .lock()
            .unwrap()
            .clone()
            .expect("fetch_for_turn must have been called with a scope");
        assert_eq!(
            scope.tenant_id, "tenant-t",
            "scope tenant_id from thread.tenant_id"
        );
        assert_eq!(
            scope.agent_id, "agent-a",
            "scope agent_id from thread.agent_id"
        );
        assert_eq!(scope.user_id, "user");
        assert_eq!(scope.project_id, thread.project_id.to_string());
    }

    // ── Phase H8.5: G.8 routing/injection re-home (gaps) ───────────────────
    //
    // The retired G.8 `step0_*` Python harness tested the full Model A step-0
    // pipeline. H8.4 deleted it + rewrote phase_f7 #1–#5/#7 to call
    // `assemble_prior_knowledge_with_hint` (re-homing the routing assertions:
    // action_short_circuit, disambiguation, orchestrator_content prose format,
    // Components arm, SplitResult). This block fills the remaining gaps (user
    // decision Q-H8.5=A1) as additive Rust unit tests on the H8.2/H8.3 fns.
    // The G.8 "injection" assertions that `assemble_prior_knowledge_with_hint`
    // structurally cannot test (N-1 prompt injection, `__llm_complete__`
    // fall-through, action-procedure execution, events/transitions, outcome
    // shaping) are agent-loop/integration concerns for a future composition
    // H.12 / agent-loop tier and are NOT re-homed here (user decision
    // Q-H8.5=B1; the legacy-shims + active_skills ones are structurally moot —
    // both deleted in H8.4/H8.4a).

    /// `RetrievalSource` whose `fetch_for_turn` always errors — used to prove
    /// the None-branch degrade path AND that the `recipe_hint` Some-branch
    /// never re-fetches.
    struct FailingRetrievalSource;

    #[async_trait]
    impl RetrievalSource for FailingRetrievalSource {
        async fn fetch_for_consumer(
            &self,
            _scope: &ComponentScope,
            _query: &str,
            _token_budget: usize,
            _consumer_tag: &str,
        ) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
            Err(RetrievalSourceError::Db(
                "test: forced fetch_for_consumer failure".to_string(),
            ))
        }

        async fn fetch_for_turn(
            &self,
            _scope: &ComponentScope,
            _query: &str,
            _token_budget: usize,
            _sender_class_code: &str,
        ) -> Result<FetchForTurnResult, RetrievalSourceError> {
            Err(RetrievalSourceError::Db(
                "test: forced fetch_for_turn failure".to_string(),
            ))
        }
    }

    /// Phase H8.5 gap #1 — Solution Override (§3.13): a `recipe_hint` carrying
    /// exactly one `override_prompt_creation` item is assembled verbatim. The
    /// item's `effective_content` becomes `orchestrator_content` unchanged
    /// (NOT prose-formatted), `override_prompt_creation` is flagged true, and
    /// `matched_component_ids` is the single-item identity set. Re-homes the
    /// override assertion from the retired G.8 `step0_*` Python harness onto
    /// the H8.2 Some-branch / `assemble_pkr_from_items` override arm.
    #[tokio::test]
    async fn phase_h8_5_solution_override_assembles_verbatim_single_item() {
        let thread = phase_f7_thread("do the thing");
        let override_item = ComponentItem {
            id: uuid::Uuid::from_u128(42),
            class_code: 12,
            prompt_uid: 42,
            name: "override-solution".to_string(),
            description: String::new(),
            effective_content: "VERBATIM SOLUTION BODY — do exactly this.".to_string(),
            override_prompt_creation: true,
            steps: None,
            allowed_tools: None,
        };
        let hint = serde_json::to_value(vec![override_item.clone()])
            .expect("serialize override ComponentItem vec");
        let result = assemble_prior_knowledge_with_hint(
            &thread,
            &thread.goal,
            TEST_TOKEN_ALLOC_2K as usize,
            "02",
            None,
            Some(hint),
        )
        .await
        .expect("override Some-branch must succeed");

        assert!(
            result.override_prompt_creation,
            "override flag must be true"
        );
        assert_eq!(
            result.orchestrator_content, override_item.effective_content,
            "override body must be verbatim, not prose-formatted"
        );
        assert_eq!(
            result.matched_component_ids,
            vec![override_item.id.to_string()],
            "identity set is the single override item"
        );
        assert!(!result.action_short_circuit);
        assert!(result.action_component_id.is_none());
        assert!(result.action_name.is_none());
        assert!(!result.disambiguation);
        assert!(result.candidates.is_empty());
        assert!(!result.tier_zero);
    }

    /// Phase H8.5 gap #2 — the `tier_zero` routing flag on the `SplitResult`
    /// arm is `!routing.llm_call_required`. The retired G.8 harness never
    /// asserted this; the rewritten phase_f7 `SplitResult` fixture uses
    /// `llm_call_required: true` (→ `tier_zero: false`) but does not assert the
    /// field. This test asserts both polarities directly on
    /// `assemble_pkr_from_fetch`.
    #[test]
    fn phase_h8_5_split_result_tier_zero_inverts_llm_call_required() {
        let skill = phase_f7_item(11, 3, "ls", "list files");

        // llm_call_required = true ⇒ Tier-1 (tier_zero false).
        let tier1 = assemble_pkr_from_fetch(FetchForTurnResult::SplitResult {
            orchestrator_items: vec![skill.clone()],
            routing: TurnRoutingSignals {
                override_prompt_creation: false,
                matched_component_ids: vec![skill.id.to_string()],
                variant_label: "default".to_string(),
                step_link: "recipe_ls#step1".to_string(),
                llm_call_required: true,
                wilson_lower: 0.0,
                tier0_eligible: true,
                recipe_id: None,
                recipe_name: "recipe_ls".to_string(),
            },
            rust_items: Vec::new(),
            instruction: None,
        });
        assert!(!tier1.tier_zero, "llm_call_required=true ⇒ tier_zero=false");

        // llm_call_required = false ⇒ Tier-0 deterministic channel (tier_zero true).
        let tier0 = assemble_pkr_from_fetch(FetchForTurnResult::SplitResult {
            orchestrator_items: vec![skill.clone()],
            routing: TurnRoutingSignals {
                override_prompt_creation: false,
                matched_component_ids: vec![skill.id.to_string()],
                variant_label: "default".to_string(),
                step_link: "recipe_ls#step1".to_string(),
                llm_call_required: false,
                wilson_lower: 0.0,
                tier0_eligible: true,
                recipe_id: None,
                recipe_name: "recipe_ls".to_string(),
            },
            rust_items: Vec::new(),
            instruction: None,
        });
        assert!(tier0.tier_zero, "llm_call_required=false ⇒ tier_zero=true");
    }

    /// Phase H8.5 gap #3 — the None-branch degrade: with no `retrieval_source`
    /// OR a failing source, `assemble_prior_knowledge_with_hint` returns the
    /// empty `PkrAssemblyResult` (no prior knowledge, no routing signals) so the
    /// Tier-1 turn proceeds without a prior-knowledge prepend.
    #[tokio::test]
    async fn phase_h8_5_no_source_or_failing_source_degrades_to_empty_pkr() {
        let thread = phase_f7_thread("anything");
        let empty = empty_pkr_assembly_result();

        // No retrieval_source ⇒ degrade.
        let no_src = assemble_prior_knowledge_with_hint(
            &thread,
            &thread.goal,
            TEST_TOKEN_ALLOC_2K as usize,
            "02",
            None,
            None,
        )
        .await
        .expect("None-branch with no source must succeed (degrade)");
        assert_eq!(no_src, empty, "no-source must degrade to the empty PKR");

        // Failing retrieval_source ⇒ degrade (fetch_for_turn Err is swallowed).
        let failing: Arc<dyn RetrievalSource> = Arc::new(FailingRetrievalSource);
        let bad_src = assemble_prior_knowledge_with_hint(
            &thread,
            &thread.goal,
            TEST_TOKEN_ALLOC_2K as usize,
            "02",
            Some(&failing),
            None,
        )
        .await
        .expect("fetch_for_turn Err must degrade, not propagate");
        assert_eq!(
            bad_src, empty,
            "failing source must degrade to the empty PKR"
        );
    }

    /// Phase H8.5 gap #4 — the `recipe_hint` Some-branch assembles the stashed
    /// orchestrator items WITHOUT re-fetching. Passing a `FailingRetrievalSource`
    /// proves the Some-branch short-circuits before `fetch_for_turn`: any
    /// re-fetch would error and degrade to empty, but the result carries the
    /// stashed items' prose + identity set.
    #[tokio::test]
    async fn phase_h8_5_recipe_hint_some_branch_assembles_stashed_items_without_refetch() {
        let thread = phase_f7_thread("list files");
        let skill = phase_f7_item(7, 3, "ls", "list files in a directory");
        let pycode = phase_f7_item(8, 22, "ls-handler", "print(result)");
        let hint = serde_json::to_value(vec![skill.clone(), pycode.clone()])
            .expect("serialize ComponentItem vec");

        // Failing source: if the Some-branch re-fetched, this would degrade.
        let src: Arc<dyn RetrievalSource> = Arc::new(FailingRetrievalSource);
        let result = assemble_prior_knowledge_with_hint(
            &thread,
            &thread.goal,
            TEST_TOKEN_ALLOC_2K as usize,
            "02",
            Some(&src),
            Some(hint),
        )
        .await
        .expect("Some-branch must succeed without re-fetch");

        assert!(!result.override_prompt_creation);
        assert!(!result.action_short_circuit);
        assert!(!result.disambiguation);
        assert!(result.candidates.is_empty());
        assert!(!result.tier_zero, "Some-branch normal assembly is Tier-1");
        assert_eq!(
            result.matched_component_ids,
            vec![skill.id.to_string(), pycode.id.to_string()],
            "identity set is the full stashed item list"
        );
        let oc = &result.orchestrator_content;
        assert!(oc.contains("## [Skill: ls]"), "prose Skill heading: {oc}");
        assert!(oc.contains("list files in a directory"), "skill body: {oc}");
        assert!(
            oc.contains("## [PythonCode: ls-handler]"),
            "prose PythonCode heading: {oc}"
        );
        assert!(oc.contains("print(result)"), "pythoncode body: {oc}");
    }
}
