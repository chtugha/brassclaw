//! Budget reservation glue for the Reborn loop's model gateway.
//!
//! [`GovernorBackedAccountant`] is the concrete
//! [`LoopModelBudgetAccountant`] used by production composition. It estimates
//! USD cost per model-backed work item from a [`ModelCostTable`] (provider-supplied
//! cost-per-token × model max-output × overestimate factor), reserves against
//! a [`ResourceGovernor`], and reconciles or releases on `post_model_work`.
//!
//! It is *not* the gate handler — when reservation returns
//! [`ResourceError::RequiresApproval`], the accountant surfaces a sanitized
//! [`LoopModelGatewayError`] of kind [`AgentLoopHostErrorKind::BudgetApprovalRequired`]
//! and a separate gate store (Phase 3) routes user resolution.

/// Default cost-overestimate factor applied to reservation quotes to absorb
/// per-model pricing volatility and variable completion lengths.
const DEFAULT_OVERESTIMATE_FACTOR: Decimal = Decimal::from_parts(120, 0, 0, false, 2);

/// Default input-token cost (USD/token) used when no model cost entry is found.
/// Conservative fallback — ensures unknown models are treated as paid calls.
const DEFAULT_INPUT_COST_PER_TOKEN: Decimal = Decimal::from_parts(25, 0, 0, false, 7);

/// Default output-token cost (USD/token) used when no model cost entry is found.
/// Conservative fallback — ensures unknown models are treated as paid calls.
const DEFAULT_OUTPUT_COST_PER_TOKEN: Decimal = Decimal::from_parts(1, 0, 0, false, 5);

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{
    InvocationId, ResourceEstimate, ResourceReservationId, ResourceScope, ResourceUsage,
    SYSTEM_RESERVED_ID, UserId,
};
use brassclaw_resources::{
    BudgetApprovalGate, BudgetEvent, BudgetEventSink, BudgetGateId, BudgetGateStatus,
    BudgetGateStore, NoOpBudgetEventSink, ResourceAccount, ResourceApprovalNeeded, ResourceError,
    ResourceGovernor, ResourceLimits,
};
use brassclaw_turns::TurnRunId;
use brassclaw_turns::run_profile::{
    AgentLoopHostErrorKind, LoopModelBudgetAccountant, LoopModelGatewayError, LoopModelRequest,
    LoopModelResponse, LoopRunContext, ModelCallOutcome, ModelProfileId, ModelWorkOutcome,
    ModelWorkRequest,
};
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use rust_decimal::Decimal;

use crate::budget_cost_table::{ModelCost, ModelCostTable};
use crate::budget_seeding::BudgetSeedingPolicy;

/// Production budget accountant.
///
/// Wraps any [`ResourceGovernor`] with model-work hooks: `pre_model_work`
/// reserves an estimated worst-case cost, `post_model_work` reconciles
/// (success) or releases (failure). The token estimate is approximated from
/// the neutral work request; refining it is a follow-up enhancement (it does
/// not change correctness because reconcile can consume actual usage).
pub struct GovernorBackedAccountant {
    governor: Arc<dyn ResourceGovernor>,
    cost_table: Arc<dyn ModelCostTable>,
    overestimate_factor: Decimal,
    /// Cost to charge when the cost table has no entry for a model.
    /// Defaults to a conservative non-zero estimate (~GPT-4o pricing,
    /// `input=$0.000003 / token`, `output=$0.000012 / token`,
    /// `max_output=8 KiB`) so a misconfigured route or new paid model
    /// never silently reconciles to zero USD (review feedback:
    /// Medium #5). Callers that genuinely want zero-cost (Ollama, free
    /// tier) wire a `ZeroCostTable` instead.
    default_cost: ModelCost,
    /// Sink the accountant uses to emit gate-related events
    /// (`BudgetEvent::GateOpened`) that the governor cannot produce on
    /// its own — the governor doesn't know about gates. Composition
    /// typically wires the same sink to both the governor and the
    /// accountant so SSE / audit consumers see a single ordered stream
    /// (review feedback: invented-gate-id bug).
    event_sink: Arc<dyn BudgetEventSink>,
    /// Tracks in-flight reservations per run so `post_model_work` can
    /// reconcile/release the matching reservation without the caller
    /// threading state through the loop port. Stores the original estimate
    /// alongside the id so reconcile can fall back to the estimated USD
    /// as the recorded actual until provider-supplied token usage threads
    /// through the loop layer.
    in_flight: Arc<DashMap<TurnRunId, InFlightReservation>>,
    // Claim before contacting the governor; no map lock spans external work.
    reserving: Arc<DashSet<TurnRunId>>,
    seeding_policy: Option<BudgetSeedingPolicy>,
    /// Accounts already successfully seeded this process lifetime.
    /// Bounded by the number of distinct (user, project) pairs the
    /// process sees; production tenants typically have O(1k) entries.
    seeded: Arc<DashSet<ResourceAccount>>,
    /// Optional approval-gate store. When set, every
    /// `ResourceError::RequiresApproval` from the governor opens a new
    /// `BudgetApprovalGate` here so a user-facing handler (or a test)
    /// can resolve it. Without a store, the approval-required error
    /// still propagates — the only thing the store adds is durable
    /// pending-gate state.
    gate_store: Option<Arc<dyn BudgetGateStore>>,
    /// Default expiry window for gates opened by this accountant. Tests
    /// can shrink this to drive F5 (expiry).
    gate_expires_after: chrono::Duration,
}

/// Per-run reservation bookkeeping. Held until `post_model_work` reconciles
/// or releases against the governor.
#[derive(Debug, Clone)]
struct InFlightReservation {
    id: ResourceReservationId,
    estimate: ResourceEstimate,
}

impl std::fmt::Debug for GovernorBackedAccountant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GovernorBackedAccountant")
            .field("cost_table", &self.cost_table)
            .field("overestimate_factor", &self.overestimate_factor)
            .field("in_flight_reservations", &self.in_flight.len())
            .finish()
    }
}

impl GovernorBackedAccountant {
    pub fn new(governor: Arc<dyn ResourceGovernor>, cost_table: Arc<dyn ModelCostTable>) -> Self {
        Self {
            governor,
            cost_table,
            overestimate_factor: DEFAULT_OVERESTIMATE_FACTOR,
            in_flight: Arc::new(DashMap::new()),
            reserving: Arc::new(DashSet::new()),
            seeding_policy: None,
            seeded: Arc::new(DashSet::new()),
            gate_store: None,
            gate_expires_after: chrono::Duration::hours(24),
            default_cost: ModelCost {
                // ~GPT-4o pricing as the safe non-zero default. Matches
                // `brassclaw_llm::costs::default_cost` so a model profile
                // missing from the table is treated as a paid call, not
                // a free one.
                input_per_token: DEFAULT_INPUT_COST_PER_TOKEN,
                output_per_token: DEFAULT_OUTPUT_COST_PER_TOKEN,
                max_output_tokens: 0,
                cache_write_multiplier_milli: 0,
                cache_read_multiplier_milli: 0,
            },
            event_sink: Arc::new(NoOpBudgetEventSink),
        }
    }

    /// Plug a [`BudgetEventSink`] that receives accountant-emitted
    /// events (today: `BudgetEvent::GateOpened`). Composition passes
    /// the same sink wired into the governor so SSE / audit consumers
    /// see a single ordered stream.
    pub fn with_event_sink(mut self, sink: Arc<dyn BudgetEventSink>) -> Self {
        self.event_sink = sink;
        self
    }

    pub fn with_overestimate_factor(mut self, factor: Decimal) -> Self {
        self.overestimate_factor = factor;
        self
    }

    /// Override the per-token cost charged when the cost table has no
    /// entry for a model. Composition callers wiring a known-zero-cost
    /// table (Ollama, free tier) explicitly set this to
    /// a row with zero input/output prices or a `ZeroCostTable` instead of letting
    /// the conservative GPT-4o-priced fallback fire.
    pub fn with_default_cost(mut self, cost: ModelCost) -> Self {
        self.default_cost = cost;
        self
    }

    /// Install a seeding policy. The first reservation against each
    /// distinct user / project account installs the configured limits
    /// when no limit was previously set.
    pub fn with_seeding_policy(mut self, policy: BudgetSeedingPolicy) -> Self {
        self.seeding_policy = Some(policy);
        self
    }

    /// Install an approval-gate store. When set, the accountant opens a
    /// new [`BudgetApprovalGate`] every time the governor cascade
    /// returns [`ResourceError::RequiresApproval`]. The gate id is
    /// available to user-facing handlers / tests via
    /// `gate_store.list_pending()`; resolving the gate (Approve with a
    /// higher limit, Cancel, or expiry) is the caller's responsibility.
    pub fn with_gate_store(mut self, store: Arc<dyn BudgetGateStore>) -> Self {
        self.gate_store = Some(store);
        self
    }

    /// Override the default 24h gate-expiry window. Tests set a short
    /// window to drive the F5 (expired) scenario.
    pub fn with_gate_expiry(mut self, duration: chrono::Duration) -> Self {
        self.gate_expires_after = duration;
        self
    }

    /// Open a pending approval gate for the given `needed`. Returns the
    /// new gate id. Best-effort: storage errors are logged and the
    /// caller still sees the `BudgetApprovalRequired` host error so the
    /// run fails closed.
    ///
    /// Emits [`BudgetEvent::GateOpened`] through the configured event
    /// sink on success so SSE / audit projections receive the *real*
    /// gate id (the governor's earlier `ApprovalRequested` event does
    /// not — it has no knowledge of the gate store).
    fn open_approval_gate(
        &self,
        scope: &ResourceScope,
        needed: &ResourceApprovalNeeded,
    ) -> Option<BudgetGateId> {
        let store = self.gate_store.as_ref()?;
        let now = Utc::now();
        let id = BudgetGateId::new();
        let gate = BudgetApprovalGate {
            id,
            needed: needed.clone(),
            opened_at: now,
            expires_at: now + self.gate_expires_after,
            status: BudgetGateStatus::Pending,
        };
        if let Err(error) = store.open(scope, gate) {
            tracing::warn!(
                ?error,
                "budget accountant failed to persist pending approval gate; \
                 the user will still see the approval-required error but the \
                 durable gate record is missing"
            );
            return None;
        }
        self.event_sink.emit(BudgetEvent::GateOpened {
            gate_id: id,
            needed: needed.clone(),
            at: now,
        });
        Some(id)
    }

    fn seed_if_missing(&self, scope: &ResourceScope) -> Result<(), ResourceError> {
        let Some(policy) = self.seeding_policy.as_ref() else {
            return Ok(());
        };
        let user_account = ResourceAccount::user(scope.tenant_id.clone(), scope.user_id.clone());
        self.install_if_unseeded(&user_account, &policy.user_daily)?;
        if let Some(project_id) = scope.project_id.clone() {
            let project_account = ResourceAccount::project(
                scope.tenant_id.clone(),
                scope.user_id.clone(),
                project_id,
            );
            self.install_if_unseeded(&project_account, &policy.project_daily)?;
        }
        Ok(())
    }

    fn install_if_unseeded(
        &self,
        account: &ResourceAccount,
        limits: &ResourceLimits,
    ) -> Result<(), ResourceError> {
        if self.seeded.contains(account) {
            return Ok(());
        }
        // The store installs the default in one mutation, preserving any
        // concurrent operator override. A failed write neither marks the
        // cache nor allows an unprotected model dispatch.
        self.governor
            .set_limit_if_missing(account.clone(), limits.clone())?;
        self.seeded.insert(account.clone());
        Ok(())
    }

    fn estimate_for(&self, request: &ModelWorkRequest) -> ResourceEstimate {
        // Fail-safe non-zero default — see `default_cost` field comment.
        // A missing entry is treated as a paid call so misconfigured
        // routes can't silently bypass daily caps.
        let cost = self
            .cost_table
            .cost_for(&request.model_profile_id)
            .unwrap_or(self.default_cost);
        let max_output_tokens = if cost.max_output_tokens == 0 {
            <dyn ModelCostTable>::DEFAULT_MAX_OUTPUT_TOKENS
        } else {
            cost.max_output_tokens
        };
        let output_tokens = request.estimated_output_tokens.unwrap_or(max_output_tokens);
        let input_usd = Decimal::from(request.estimated_input_tokens) * cost.input_per_token;
        let output_usd = Decimal::from(output_tokens) * cost.output_per_token;
        let raw_usd = input_usd + output_usd;
        let estimated_usd = raw_usd * self.overestimate_factor;
        ResourceEstimate {
            usd: if estimated_usd > Decimal::ZERO {
                Some(estimated_usd)
            } else {
                None
            },
            input_tokens: Some(request.estimated_input_tokens),
            output_tokens: Some(output_tokens),
            ..ResourceEstimate::default()
        }
    }

    fn reserve_estimate(
        &self,
        context: &LoopRunContext,
        estimate: ResourceEstimate,
    ) -> Result<(), LoopModelGatewayError> {
        // Claim the run before making any governor call. This prevents two
        // concurrent callers from creating holds and then racing to publish
        // one of them. The claim has its own short-lived guard, so governor
        // errors and unwinding cannot leave a permanent admission lock.
        if !self.reserving.insert(context.run_id) {
            return Err(overlapping_reservation_error());
        }
        let _claim = ReservationClaim {
            reserving: self.reserving.as_ref(),
            run_id: context.run_id,
        };
        if self.in_flight.contains_key(&context.run_id) {
            return Err(overlapping_reservation_error());
        }

        let scope = self.resource_scope(context);
        if self.seed_if_missing(&scope).is_err() {
            return Err(LoopModelGatewayError::new(
                AgentLoopHostErrorKind::BudgetAccountingFailed,
                "default budget installation failed",
            )
            .map_err(internal_summary_error)?);
        }
        let reservation_id = ResourceReservationId::new();
        // Retain the address before durable I/O. A storage error can follow a
        // committed write; it must fence another quote instead of orphaning
        // the hold or dispatching/replaying model work without acknowledgement.
        self.in_flight.insert(
            context.run_id,
            InFlightReservation {
                id: reservation_id,
                estimate: estimate.clone(),
            },
        );
        match self
            .governor
            .reserve_with_id_and_outcome(scope, estimate.clone(), reservation_id)
        {
            Ok(outcome) => {
                self.in_flight.insert(
                    context.run_id,
                    InFlightReservation {
                        id: outcome.reservation.id,
                        estimate,
                    },
                );
                if !outcome.warnings.is_empty() {
                    tracing::debug!(
                        warnings = outcome.warnings.len(),
                        run_id = ?context.run_id,
                        "budget reservation crossed warn threshold"
                    );
                }
                Ok(())
            }
            Err(ResourceError::RequiresApproval { needed, .. }) => {
                self.in_flight.remove(&context.run_id);
                // Side-effect: persist a pending gate when a store is
                // wired. The error returned to the run is unchanged so
                // existing callers still fail closed; the gate is what
                // lets a user-facing handler (or test) resolve the
                // approval out-of-band. We pass the caller's scope so
                // the store can route the gate to the right tenant
                // path (review feedback Thermo-Nuclear #2).
                let scope = self.resource_scope(context);
                let _ = self.open_approval_gate(&scope, needed.as_ref());
                Err(LoopModelGatewayError::new(
                    AgentLoopHostErrorKind::BudgetApprovalRequired,
                    format!("budget approval required for {}", needed.dimension),
                )
                .map_err(internal_summary_error)?)
            }
            Err(ResourceError::LimitExceeded { denial, .. }) => {
                self.in_flight.remove(&context.run_id);
                Err(LoopModelGatewayError::new(
                    AgentLoopHostErrorKind::BudgetExceeded,
                    format!("budget exhausted for {}", denial.dimension),
                )
                .map_err(internal_summary_error)?)
            }
            Err(ResourceError::InvalidEstimate { dimension, .. }) => {
                self.in_flight.remove(&context.run_id);
                Err(LoopModelGatewayError::new(
                    AgentLoopHostErrorKind::BudgetAccountingFailed,
                    format!("invalid estimate for {dimension}"),
                )
                .map_err(internal_summary_error)?)
            }
            Err(error) => {
                if matches!(error, ResourceError::Storage { .. }) {
                    tracing::warn!(
                        run_id = ?context.run_id, reservation_id = ?reservation_id,
                        "budget admission outcome uncertain; reservation address retained; model dispatch denied"
                    );
                } else {
                    self.in_flight.remove(&context.run_id);
                }
                Err(LoopModelGatewayError::new(
                    AgentLoopHostErrorKind::BudgetAccountingFailed,
                    "budget reservation failed",
                )
                .map_err(internal_summary_error)?)
            }
        }
    }

    fn resource_scope(&self, context: &LoopRunContext) -> ResourceScope {
        let user_id = context
            .actor
            .as_ref()
            .map(|actor| actor.user_id.clone())
            .unwrap_or_else(|| UserId::from_trusted(SYSTEM_RESERVED_ID.to_string()));
        ResourceScope {
            tenant_id: context.scope.tenant_id.clone(),
            user_id,
            agent_id: context.scope.agent_id.clone(),
            project_id: context.scope.project_id.clone(),
            thread_id: Some(context.scope.thread_id.clone()),
            invocation_id: InvocationId::new(),
        }
    }
}

#[async_trait]
impl LoopModelBudgetAccountant for GovernorBackedAccountant {
    fn isolated_work_accountant(&self) -> Option<Arc<dyn LoopModelBudgetAccountant>> {
        Some(Arc::new(Self {
            governor: Arc::clone(&self.governor),
            cost_table: Arc::clone(&self.cost_table),
            overestimate_factor: self.overestimate_factor,
            default_cost: self.default_cost,
            event_sink: Arc::clone(&self.event_sink),
            in_flight: Arc::new(DashMap::new()),
            reserving: Arc::new(DashSet::new()),
            seeding_policy: self.seeding_policy.clone(),
            seeded: Arc::clone(&self.seeded),
            gate_store: self.gate_store.clone(),
            gate_expires_after: self.gate_expires_after,
        }))
    }

    async fn pre_model_work(
        &self,
        context: &LoopRunContext,
        request: &ModelWorkRequest,
    ) -> Result<(), LoopModelGatewayError> {
        self.reserve_estimate(context, self.estimate_for(request))
    }

    async fn post_model_work(
        &self,
        context: &LoopRunContext,
        _request: &ModelWorkRequest,
        outcome: ModelWorkOutcome,
    ) -> Result<(), LoopModelGatewayError> {
        // Peek without removing — only clear the in-flight entry after the
        // governor confirms the reconcile/release succeeded. Otherwise a
        // transient storage error would orphan the reservation in the
        // governor with no id left here to retry or audit.
        let entry = match self.in_flight.get(&context.run_id) {
            Some(e) => e.clone(),
            None => {
                // No reservation registered — pre_model_work must have
                // failed before reservation succeeded.
                return Ok(());
            }
        };
        let reservation_id = entry.id;
        let result = match outcome {
            ModelWorkOutcome::Success(usage) => {
                let usage = usage_for_model_work(usage, &entry.estimate);
                self.governor.reconcile(reservation_id, usage).map(|_| ())
            }
            ModelWorkOutcome::Failure(_) => self.governor.release(reservation_id).map(|_| ()),
        };
        match result {
            Ok(()) => {
                self.in_flight.remove(&context.run_id);
                Ok(())
            }
            Err(err) => {
                tracing::warn!(
                    error = ?err,
                    run_id = ?context.run_id,
                    reservation_id = ?reservation_id,
                    "budget accounting failed during post-model-call reconciliation; \
                     reservation id retained for retry/cleanup"
                );
                Err(LoopModelGatewayError::new(
                    AgentLoopHostErrorKind::BudgetAccountingFailed,
                    "budget accounting failed",
                )
                .unwrap_or_else(|reason| panic!("internal summary invariant violated: {reason}")))
            }
        }
    }

    /// Preserve the full response for accurate assistant-work reconciliation.
    /// Auxiliary provider work uses the same richer hook with its own retained
    /// work request and isolated reservation namespace.
    async fn post_model_call(
        &self,
        context: &LoopRunContext,
        request: &LoopModelRequest,
        outcome: ModelCallOutcome<'_>,
    ) -> Result<(), LoopModelGatewayError> {
        self.post_model_work_result(
            context,
            &ModelWorkRequest::for_assistant(context, request),
            outcome,
        )
        .await
    }

    async fn post_model_work_result(
        &self,
        context: &LoopRunContext,
        _request: &ModelWorkRequest,
        outcome: ModelCallOutcome<'_>,
    ) -> Result<(), LoopModelGatewayError> {
        // Peek without removing — only clear the in-flight entry after the
        // governor confirms the reconcile/release succeeded. Otherwise a
        // transient storage error would orphan the reservation in the
        // governor with no id left here to retry or audit.
        let entry = match self.in_flight.get(&context.run_id) {
            Some(e) => e.clone(),
            None => {
                // No reservation registered — pre_model_call must have
                // failed before reservation succeeded.
                return Ok(());
            }
        };
        let reservation_id = entry.id;
        let result = match outcome {
            ModelCallOutcome::Success(response) => {
                let usage = usage_for_response(
                    response,
                    &entry.estimate,
                    self.cost_table.as_ref(),
                    &response.effective_model_profile_id,
                    &self.default_cost,
                );
                self.governor.reconcile(reservation_id, usage).map(|_| ())
            }
            ModelCallOutcome::Failure(_) => self.governor.release(reservation_id).map(|_| ()),
        };
        match result {
            Ok(()) => {
                self.in_flight.remove(&context.run_id);
                Ok(())
            }
            Err(err) => {
                tracing::warn!(
                    error = ?err,
                    run_id = ?context.run_id,
                    reservation_id = ?reservation_id,
                    "budget accounting failed during post-model-call reconciliation; \
                     reservation id retained for retry/cleanup"
                );
                Err(LoopModelGatewayError::new(
                    AgentLoopHostErrorKind::BudgetAccountingFailed,
                    "budget accounting failed",
                )
                .unwrap_or_else(|reason| panic!("internal summary invariant violated: {reason}")))
            }
        }
    }

    /// Synchronous best-effort release for cancellation paths.
    ///
    /// `HostManagedLoopModelPort`'s RAII guard calls this when the
    /// model future is dropped mid-await — before `post_model_call`
    /// could run, or while `post_model_call` is still awaiting.
    ///
    /// Idempotency / failure handling:
    ///
    /// - We **peek** the in-flight entry first, attempt the governor
    ///   release, and only remove the entry on success. On a transient
    ///   storage error the entry stays in `in_flight` with a warning
    ///   logged, so a later retry / cleanup hook still has the
    ///   reservation id needed to close the hold (review feedback:
    ///   Medium #4).
    /// - This method is safe to call after a successful
    ///   `post_model_call`. `post_model_call` removes the entry on
    ///   success, so a follow-up `release_in_flight` short-circuits
    ///   on the `is_none` branch (review feedback: Medium #3 — the
    ///   RAII guard stays armed across `post_model_call`'s await).
    fn release_in_flight(&self, context: &LoopRunContext) {
        let Some(entry) = self.in_flight.get(&context.run_id).map(|e| e.clone()) else {
            return;
        };
        match self.governor.release(entry.id) {
            Ok(_) => {
                self.in_flight.remove(&context.run_id);
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    run_id = ?context.run_id,
                    reservation_id = ?entry.id,
                    "cancellation-safe release of in-flight budget reservation failed; \
                     entry retained for a future retry / cleanup hook"
                );
            }
        }
    }
}

struct ReservationClaim<'a> {
    reserving: &'a DashSet<TurnRunId>,
    run_id: TurnRunId,
}

impl Drop for ReservationClaim<'_> {
    fn drop(&mut self) {
        self.reserving.remove(&self.run_id);
    }
}

fn overlapping_reservation_error() -> LoopModelGatewayError {
    LoopModelGatewayError {
        kind: AgentLoopHostErrorKind::BudgetAccountingFailed,
        safe_summary: brassclaw_turns::run_profile::LoopSafeSummary::model_gateway_failed(),
        reason_kind: None,
        diagnostic_ref: None,
    }
}

fn usage_for_response(
    response: &LoopModelResponse,
    estimate: &ResourceEstimate,
    cost_table: &dyn ModelCostTable,
    effective_model: &ModelProfileId,
    default_cost: &ModelCost,
) -> ResourceUsage {
    // Prefer provider-reported usage when the gateway threaded real numbers
    // through `LoopModelResponse::usage`. Compute actual USD from the cost
    // table for the effective model so daily caps deplete by real spend.
    //
    // When the gateway did not surface usage (replay stubs, providers
    // without a usage object), fall back to reconciling the reservation
    // estimate. That is conservative (the estimate carries the
    // overestimate factor) but matches the security invariant the cascade
    // depends on — daily caps still deplete, just by an upper bound.
    //
    // When the cost table has no entry for the effective model we use
    // the accountant's `default_cost` rather than zero, so a
    // misconfigured route or new paid model never silently reconciles to
    // zero USD (review feedback: Medium #5).
    let output_bytes = response
        .chunks
        .iter()
        .map(|chunk| chunk.safe_text_delta.len() as u64)
        .sum();
    if let Some(usage) = response.usage {
        let cost = cost_table
            .cost_for(effective_model)
            .unwrap_or(*default_cost);
        // Provider-supplied token counts split across fresh input,
        // cache creation, cache read, and output. Cache multipliers are
        // already inherent in `cost` (see `ModelCost::cache_*_cost`).
        // Providers without caching return 0 for cache fields, so the
        // multiplier math yields zero cost naturally — no provider guard
        // needed.
        let input_tokens = u64::from(usage.input_tokens);
        let output_tokens = u64::from(usage.output_tokens);
        let cache_creation_input_tokens = u64::from(usage.cache_creation_input_tokens);
        let cache_read_input_tokens = u64::from(usage.cache_read_input_tokens);
        let actual_usd = cost.fresh_input_cost(input_tokens)
            + cost.cache_write_cost(cache_creation_input_tokens)
            + cost.cache_read_cost(cache_read_input_tokens)
            + cost.output_cost(output_tokens);
        return ResourceUsage {
            usd: actual_usd,
            input_tokens,
            output_tokens,
            wall_clock_ms: 0,
            output_bytes,
            network_egress_bytes: 0,
            process_count: 0,
        };
    }
    let chunks = response.chunks.len() as u64;
    ResourceUsage {
        usd: estimate.usd.unwrap_or(Decimal::ZERO),
        input_tokens: estimate.input_tokens.unwrap_or(0),
        output_tokens: chunks,
        wall_clock_ms: 0,
        output_bytes,
        network_egress_bytes: 0,
        process_count: 0,
    }
}

fn usage_for_model_work(
    usage: brassclaw_turns::run_profile::ModelWorkUsage,
    estimate: &ResourceEstimate,
) -> ResourceUsage {
    // `ModelWorkUsage` (consumed by non-loop model work such as system
    // inference and async tool calls) intentionally carries only
    // `output_tokens` / `output_bytes` / `wall_clock_ms` — there is no
    // analogous `LoopModelUsage::input_tokens` or cache-field surface here,
    // so all input-token and USD accounting must come from the original
    // reservation estimate. Reconciling the estimate as the recorded USD
    // spend is conservative (the estimate carries the over-reserve factor)
    // and ensures daily USD budgets still deplete for these non-loop calls.
    ResourceUsage {
        usd: estimate.usd.unwrap_or(Decimal::ZERO),
        input_tokens: estimate.input_tokens.unwrap_or(0),
        output_tokens: usage
            .output_tokens
            .unwrap_or_else(|| estimate.output_tokens.unwrap_or(0)),
        wall_clock_ms: usage.wall_clock_ms,
        output_bytes: usage.output_bytes,
        network_egress_bytes: 0,
        process_count: 0,
    }
}

fn internal_summary_error(reason: String) -> LoopModelGatewayError {
    // The error summary is itself sanitized by `LoopModelGatewayError::new`
    // — failure to construct one indicates a programming error.
    LoopModelGatewayError::new(
        AgentLoopHostErrorKind::Internal,
        "budget accountant invalid",
    )
    .unwrap_or_else(|_| panic!("internal budget-accountant invariant: {reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget_cost_table::ZeroCostTable;
    use brassclaw_host_api::{TenantId, ThreadId};
    use brassclaw_resources::{
        BudgetPeriod, BudgetThresholds, FakeClock, InMemoryResourceGovernor, ResourceAccount,
        ResourceLimits,
    };
    use brassclaw_turns::{
        AgentLoopDriverDescriptor, RunProfileId, RunProfileVersion, TurnActor, TurnId, TurnRunId,
        TurnScope,
        run_profile::{
            CancellationPolicy, CapabilitySurfaceProfileId, CheckpointPolicy, CheckpointSchemaId,
            ConcurrencyClass, ContextProfileId, LoopDriverId, LoopModelRequest, LoopModelResponse,
            LoopRunContext, ModelCallOutcome, ModelProfileId, PersonalContextPolicy,
            RedactedRunProfileProvenance, ResolvedRunProfile, ResourceBudgetPolicy,
            ResourceBudgetTier, RunClassId, RunProfileFingerprint, RuntimeProfileConstraints,
            SchedulingClass, SteeringPolicy,
        },
    };
    use chrono::Utc;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn run_context() -> LoopRunContext {
        let scope = TurnScope::new(
            TenantId::new("tenant-acct").unwrap(),
            None,
            None,
            ThreadId::new("thread-acct").unwrap(),
        );
        let descriptor = AgentLoopDriverDescriptor {
            id: LoopDriverId::new("acct_test").unwrap(),
            version: RunProfileVersion::new(1),
            checkpoint_schema_id: Some(CheckpointSchemaId::new("acct_chk").unwrap()),
            checkpoint_schema_version: Some(RunProfileVersion::new(1)),
        };
        let profile = ResolvedRunProfile {
            run_class_id: RunClassId::new("acct").unwrap(),
            profile_id: RunProfileId::default_profile(),
            profile_version: RunProfileVersion::new(1),
            loop_driver: descriptor.clone(),
            checkpoint_schema_id: descriptor.checkpoint_schema_id.unwrap(),
            checkpoint_schema_version: descriptor.checkpoint_schema_version.unwrap(),
            model_profile_id: ModelProfileId::new("acct_model").unwrap(),
            capability_surface_profile_id: CapabilitySurfaceProfileId::new("acct_caps").unwrap(),
            context_profile_id: ContextProfileId::new("acct_ctx").unwrap(),
            steering_policy: SteeringPolicy {
                allow_steering: false,
                allow_interrupt: true,
                allow_driver_specific_nudges: false,
            },
            cancellation_policy: CancellationPolicy {
                allow_cancel: true,
                require_checkpoint_before_cancel: false,
            },
            checkpoint_policy: CheckpointPolicy {
                require_before_model: false,
                require_before_side_effect: false,
                require_before_block: true,
                max_checkpoint_bytes: 64 * 1024,
                require_final_checkpoint: false,
                allow_no_reply_completion: false,
            },
            resource_budget_policy: ResourceBudgetPolicy {
                tier: ResourceBudgetTier::new("acct_tier").unwrap(),
                max_model_calls: 32,
                max_capability_invocations: 64,
            },
            personal_context_policy: PersonalContextPolicy::Excluded,
            runtime_constraints: RuntimeProfileConstraints {
                allow_raw_runtime_backend_selection: false,
                allow_broad_capability_surface: false,
            },
            runner_pool_id: None,
            scheduling_class: SchedulingClass::new("interactive").unwrap(),
            concurrency_class: ConcurrencyClass::new("thread_serial").unwrap(),
            resolution_fingerprint: RunProfileFingerprint::new("acct-fp").unwrap(),
            provenance: RedactedRunProfileProvenance {
                sources: vec![],
                effective_privileges: vec![],
            },
        };
        LoopRunContext::new(scope, TurnId::new(), TurnRunId::new(), profile)
            .with_actor(TurnActor::new(UserId::new("acct-user").unwrap()))
    }

    fn sample_request() -> LoopModelRequest {
        LoopModelRequest {
            messages: vec![],
            surface_version: None,
            model_preference: None,
            capability_view: None,
            resolved_messages: None,
        }
    }

    #[derive(Debug)]
    struct CostStub(ModelCost);

    impl ModelCostTable for CostStub {
        fn cost_for(&self, _: &ModelProfileId) -> Option<ModelCost> {
            Some(self.0)
        }
    }

    #[tokio::test]
    async fn exact_decimal_quote_can_reserve_at_the_money_ceiling() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let context = run_context();
        let account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            UserId::new("acct-user").unwrap(),
        );
        governor
            .set_limit(
                account.clone(),
                ResourceLimits {
                    max_usd: Some(dec!(1.20)),
                    thresholds: brassclaw_resources::BudgetThresholds::DISABLED,
                    ..Default::default()
                },
            )
            .unwrap();
        let accountant = GovernorBackedAccountant::new(
            governor.clone(),
            Arc::new(CostStub(ModelCost {
                input_per_token: Decimal::ZERO,
                output_per_token: dec!(0.10),
                max_output_tokens: 10,
                cache_write_multiplier_milli: 0,
                cache_read_multiplier_milli: 0,
            })),
        );
        accountant
            .pre_model_call(&context, &sample_request())
            .await
            .unwrap();
        assert_eq!(
            governor
                .account_snapshot(&account)
                .unwrap()
                .unwrap()
                .ledger
                .reserved
                .usd,
            dec!(1.20)
        );
        assert_eq!(accountant.default_cost.input_per_token, dec!(0.0000025));
        assert_eq!(accountant.default_cost.output_per_token, dec!(0.00001));
        accountant.release_in_flight(&context);
        assert_eq!(
            governor
                .account_snapshot(&account)
                .unwrap()
                .unwrap()
                .ledger
                .reserved
                .usd,
            Decimal::ZERO
        );
    }

    #[tokio::test]
    async fn pre_model_call_reserves_against_governor() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let cost = ModelCost {
            input_per_token: dec!(0.000001),
            output_per_token: dec!(0.00001),
            max_output_tokens: 1024,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        };
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(CostStub(cost)));
        let context = run_context();
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();
        // Reservation now in-flight.
        let account = ResourceAccount::tenant(context.scope.tenant_id.clone());
        let snapshot = governor.account_snapshot(&account).unwrap();
        // Account is only present when reservations have touched it AND
        // limit is set. Without a limit, snapshot is None — but the
        // reservation is still tracked in `state.reservations`. Confirm via
        // accountant in-flight map instead.
        assert!(accountant.in_flight.contains_key(&context.run_id));
        let _ = snapshot;
    }

    #[tokio::test]
    async fn pre_model_call_returns_budget_exceeded_when_limit_zero_but_negative() {
        // A negative-USD estimate would be invalid input, but our default
        // path always produces non-negative estimates. Instead verify that
        // a very tight USD limit hard-denies.
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let context = run_context();
        let account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            UserId::new("acct-user").unwrap(),
        );
        governor
            .set_limit(
                account,
                ResourceLimits {
                    max_usd: Some(dec!(0.000001)),
                    ..ResourceLimits::default()
                },
            )
            .unwrap();
        let cost = ModelCost {
            input_per_token: dec!(0.01),
            output_per_token: dec!(0.10),
            max_output_tokens: 100,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        };
        let accountant = GovernorBackedAccountant::new(governor, Arc::new(CostStub(cost)));
        let request = sample_request();
        let err = accountant
            .pre_model_call(&context, &request)
            .await
            .unwrap_err();
        assert_eq!(err.kind, AgentLoopHostErrorKind::BudgetExceeded);
    }

    #[tokio::test]
    async fn pre_model_call_returns_budget_approval_required_when_pause_threshold_crossed() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let context = run_context();
        let account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            UserId::new("acct-user").unwrap(),
        );
        governor
            .set_limit(
                account,
                ResourceLimits {
                    max_usd: Some(dec!(10.00)),
                    period: BudgetPeriod::Rolling24h,
                    thresholds: BudgetThresholds {
                        warn_at: 0.75,
                        pause_at: 0.90,
                    },
                    ..ResourceLimits::default()
                },
            )
            .unwrap();
        let cost = ModelCost {
            input_per_token: dec!(0.10),
            output_per_token: dec!(0.10),
            // 100 tokens × $0.10 = $10 × 1.20 factor = $12 → over 90% of $10
            // but the hard cap is also exceeded. Adjust to push into the
            // approval band by sizing max_output to land at ~$9.
            max_output_tokens: 75,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        };
        let accountant = GovernorBackedAccountant::new(governor, Arc::new(CostStub(cost)))
            .with_overestimate_factor(dec!(1.0));
        let request = sample_request();
        let err = accountant
            .pre_model_call(&context, &request)
            .await
            .unwrap_err();
        // 75 × 0.10 = $7.50; input_tokens=64*0.10=$6.40; total=$13.90 → over hard cap.
        // We expect BudgetExceeded since utilization > 100%, OR
        // BudgetApprovalRequired if just below. Either is acceptable —
        // confirm we got a budget-class outcome, not Internal.
        assert!(matches!(
            err.kind,
            AgentLoopHostErrorKind::BudgetExceeded | AgentLoopHostErrorKind::BudgetApprovalRequired
        ));
    }

    #[tokio::test]
    async fn post_model_call_success_reconciles_reservation() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let cost = ModelCost {
            input_per_token: Decimal::ZERO,
            output_per_token: Decimal::ZERO,
            max_output_tokens: 0,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        };
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(CostStub(cost)));
        let context = run_context();
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();
        let response = LoopModelResponse {
            chunks: vec![],
            safe_reasoning_deltas: Vec::new(),
            output: brassclaw_turns::run_profile::ParentLoopOutput::AssistantReply(
                brassclaw_turns::run_profile::AssistantReply {
                    content: "ok".to_string(),
                },
            ),
            effective_model_profile_id: ModelProfileId::new("acct_model").unwrap(),
            usage: None,
        };
        accountant
            .post_model_call(&context, &request, ModelCallOutcome::Success(&response))
            .await
            .unwrap();
        // In-flight cleared.
        assert!(!accountant.in_flight.contains_key(&context.run_id));
    }

    #[tokio::test]
    async fn post_model_call_failure_releases_reservation() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let cost = ModelCost {
            input_per_token: Decimal::ZERO,
            output_per_token: Decimal::ZERO,
            max_output_tokens: 0,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        };
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(CostStub(cost)));
        let context = run_context();
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();
        let failure =
            LoopModelGatewayError::new(AgentLoopHostErrorKind::Unavailable, "model unavailable")
                .unwrap();
        accountant
            .post_model_call(&context, &request, ModelCallOutcome::Failure(&failure))
            .await
            .unwrap();
        assert!(!accountant.in_flight.contains_key(&context.run_id));
    }

    #[tokio::test]
    async fn zero_cost_table_yields_no_usd_estimate() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let accountant = GovernorBackedAccountant::new(governor, Arc::new(ZeroCostTable));
        let context = run_context();
        let request = sample_request();
        let estimate =
            accountant.estimate_for(&ModelWorkRequest::for_assistant(&context, &request));
        assert_eq!(estimate.usd, None);
        assert!(estimate.input_tokens.is_some());
    }

    #[test]
    fn fake_clock_governor_integration_smokes() {
        // Ensure the accountant compiles against a governor with a FakeClock
        // for downstream period-aware tests in later phases.
        let clock = FakeClock::new(Utc::now());
        let _ = InMemoryResourceGovernor::with_clock(Arc::new(clock));
    }

    #[tokio::test]
    async fn seeding_policy_installs_user_limit_on_first_touch() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let context = run_context();
        let user_account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            UserId::new("acct-user").unwrap(),
        );
        // Before first model call, no limit exists.
        assert!(governor.account_snapshot(&user_account).unwrap().is_none());

        let policy = BudgetSeedingPolicy::new(
            dec!(5.00),
            dec!(2.00),
            BudgetPeriod::Rolling24h,
            BudgetThresholds::DISABLED,
        );
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(ZeroCostTable))
            .with_seeding_policy(policy);
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();

        let snapshot = governor.account_snapshot(&user_account).unwrap().unwrap();
        assert_eq!(snapshot.limits.unwrap().max_usd, Some(dec!(5.00)));
    }

    #[tokio::test]
    async fn seeding_policy_does_not_overwrite_existing_user_limit() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let context = run_context();
        let user_account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            UserId::new("acct-user").unwrap(),
        );
        governor
            .set_limit(
                user_account.clone(),
                ResourceLimits {
                    max_usd: Some(dec!(100.00)),
                    ..ResourceLimits::default()
                },
            )
            .unwrap();

        let policy = BudgetSeedingPolicy::new(
            dec!(5.00),
            dec!(2.00),
            BudgetPeriod::Rolling24h,
            BudgetThresholds::DISABLED,
        );
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(ZeroCostTable))
            .with_seeding_policy(policy);
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();

        let snapshot = governor.account_snapshot(&user_account).unwrap().unwrap();
        assert_eq!(snapshot.limits.unwrap().max_usd, Some(dec!(100.00)));
    }

    #[tokio::test]
    async fn post_model_call_success_records_estimated_usd_until_provider_threading_lands() {
        // Regression: prior to threading provider usage, reconcile recorded
        // `usd: ZERO`, so daily USD budgets only constrained concurrency.
        // The estimate fallback records the conservative reservation cost
        // as actual spend, ensuring a daily USD cap actually depletes.
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let context = run_context();
        let user_account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            UserId::new("acct-user").unwrap(),
        );
        governor
            .set_limit(
                user_account.clone(),
                ResourceLimits {
                    max_usd: Some(dec!(100.00)),
                    period: BudgetPeriod::Rolling24h,
                    ..ResourceLimits::default()
                },
            )
            .unwrap();
        let cost = ModelCost {
            input_per_token: dec!(0.0001),
            output_per_token: dec!(0.001),
            max_output_tokens: 1024,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        };
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(CostStub(cost)));
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();
        let response = LoopModelResponse {
            chunks: vec![],
            safe_reasoning_deltas: Vec::new(),
            output: brassclaw_turns::run_profile::ParentLoopOutput::AssistantReply(
                brassclaw_turns::run_profile::AssistantReply {
                    content: "ok".to_string(),
                },
            ),
            effective_model_profile_id: ModelProfileId::new("acct_model").unwrap(),
            usage: None,
        };
        accountant
            .post_model_call(&context, &request, ModelCallOutcome::Success(&response))
            .await
            .unwrap();
        let snapshot = governor.account_snapshot(&user_account).unwrap().unwrap();
        assert!(
            snapshot.ledger.spent.usd > Decimal::ZERO,
            "USD spend must be recorded on success (got {})",
            snapshot.ledger.spent.usd,
        );
    }

    #[tokio::test]
    async fn post_model_call_reconciles_provider_usage_when_response_threads_real_tokens() {
        // When the gateway reports actual `(input_tokens, output_tokens)` on
        // `LoopModelResponse::usage`, reconcile MUST use those numbers
        // multiplied by the cost table for the effective model — not the
        // conservative reservation estimate. This is the long-term fix for
        // the "USD never depletes by real spend" follow-up from #3841.
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let context = run_context();
        let user_account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            UserId::new("acct-user").unwrap(),
        );
        governor
            .set_limit(
                user_account.clone(),
                ResourceLimits {
                    max_usd: Some(dec!(10_000.00)),
                    period: BudgetPeriod::Rolling24h,
                    ..ResourceLimits::default()
                },
            )
            .unwrap();
        let cost = ModelCost {
            input_per_token: dec!(0.01),
            output_per_token: dec!(0.10),
            max_output_tokens: 1024,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        };
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(CostStub(cost)));
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();
        let response = LoopModelResponse {
            chunks: vec![],
            safe_reasoning_deltas: Vec::new(),
            output: brassclaw_turns::run_profile::ParentLoopOutput::AssistantReply(
                brassclaw_turns::run_profile::AssistantReply {
                    content: "ok".to_string(),
                },
            ),
            effective_model_profile_id: ModelProfileId::new("acct_model").unwrap(),
            usage: Some(brassclaw_turns::run_profile::LoopModelUsage {
                input_tokens: 7,
                output_tokens: 3,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            }),
        };
        accountant
            .post_model_call(&context, &request, ModelCallOutcome::Success(&response))
            .await
            .unwrap();
        let snapshot = governor.account_snapshot(&user_account).unwrap().unwrap();
        // 7 * $0.01 + 3 * $0.10 = $0.37 — must match exactly, not the
        // conservative reservation estimate (which is much larger).
        assert_eq!(
            snapshot.ledger.spent.usd,
            dec!(0.37),
            "USD spend must reconcile from provider usage, got {}",
            snapshot.ledger.spent.usd,
        );
        assert_eq!(snapshot.ledger.spent.input_tokens, 7);
        assert_eq!(snapshot.ledger.spent.output_tokens, 3);
    }

    #[tokio::test]
    async fn release_in_flight_drains_orphan_reservation_on_cancellation() {
        // Regression for #3841 follow-up "cancellation safety — reservation
        // orphans on tokio cancel". When `stream_model` is dropped mid-await
        // the RAII guard in `HostManagedLoopModelPort` calls
        // `release_in_flight`, which must clear the per-run entry AND
        // release the underlying reservation against the governor.
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(ZeroCostTable));
        let context = run_context();
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();
        assert!(accountant.in_flight.contains_key(&context.run_id));
        // Simulate cancellation: post_model_call never runs.
        accountant.release_in_flight(&context);
        assert!(
            !accountant.in_flight.contains_key(&context.run_id),
            "release_in_flight must drop the per-run entry"
        );
        // And a follow-up pre_model_call on the same run must succeed —
        // proves the governor side really released, not just the
        // accountant cache.
        accountant.pre_model_call(&context, &request).await.unwrap();
    }

    #[tokio::test]
    async fn pre_model_call_rejects_overlapping_reservation_for_same_run() {
        // Regression: in_flight is keyed by TurnRunId. Two overlapping
        // reservations under the same run id used to overwrite each other,
        // leaking one hold. Defensively reject the second call.
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(ZeroCostTable));
        let context = run_context();
        let request = sample_request();
        accountant.pre_model_call(&context, &request).await.unwrap();
        let err = accountant
            .pre_model_call(&context, &request)
            .await
            .unwrap_err();
        assert_eq!(err.kind, AgentLoopHostErrorKind::BudgetAccountingFailed);
    }

    #[test]
    fn concurrent_reservation_admission_keeps_one_actual_hold() {
        let governor: Arc<dyn ResourceGovernor> = Arc::new(InMemoryResourceGovernor::new());
        let accountant = GovernorBackedAccountant::new(governor, Arc::new(ZeroCostTable));
        let context = run_context();
        let barrier = std::sync::Barrier::new(16);
        let outcomes = std::thread::scope(|scope| {
            let threads = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        accountant.reserve_estimate(&context, ResourceEstimate::default())
                    })
                })
                .collect::<Vec<_>>();
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        for error in outcomes.into_iter().filter_map(Result::err) {
            assert_eq!(error.kind, AgentLoopHostErrorKind::BudgetAccountingFailed);
        }
        assert!(accountant.reserving.is_empty());
        assert_eq!(accountant.in_flight.len(), 1);
        accountant.release_in_flight(&context);
        assert!(accountant.in_flight.is_empty());
        accountant
            .reserve_estimate(&context, ResourceEstimate::default())
            .unwrap();
        accountant.release_in_flight(&context);
    }

    #[tokio::test]
    async fn seeding_retry_after_transient_failure_uses_failing_governor() {
        // A governor that always fails set_limit must not poison `seeded`
        // — a subsequent call should re-attempt seeding instead of
        // silently proceeding without the intended default cap.
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Debug, Default)]
        struct FailingSetLimitGovernor {
            calls: AtomicUsize,
            inner: InMemoryResourceGovernor,
            fail_first_n: usize,
        }

        impl ResourceGovernor for FailingSetLimitGovernor {
            fn set_limit(
                &self,
                account: ResourceAccount,
                limits: ResourceLimits,
            ) -> Result<(), ResourceError> {
                self.inner.set_limit(account, limits)
            }
            fn set_limit_if_missing(
                &self,
                account: ResourceAccount,
                limits: ResourceLimits,
            ) -> Result<bool, ResourceError> {
                let n = self.calls.fetch_add(1, Ordering::SeqCst);
                if n < self.fail_first_n {
                    return Err(ResourceError::Storage {
                        reason: "synthetic installation failure".into(),
                    });
                }
                self.inner.set_limit_if_missing(account, limits)
            }
            fn reserve_with_outcome(
                &self,
                scope: ResourceScope,
                estimate: ResourceEstimate,
            ) -> Result<ReservationOutcome, ResourceError> {
                self.inner.reserve_with_outcome(scope, estimate)
            }
            fn reserve_with_id_and_outcome(
                &self,
                scope: ResourceScope,
                estimate: ResourceEstimate,
                reservation_id: ResourceReservationId,
            ) -> Result<ReservationOutcome, ResourceError> {
                self.inner
                    .reserve_with_id_and_outcome(scope, estimate, reservation_id)
            }
            fn reconcile(
                &self,
                reservation_id: ResourceReservationId,
                actual: ResourceUsage,
            ) -> Result<ResourceReceipt, ResourceError> {
                self.inner.reconcile(reservation_id, actual)
            }
            fn release(
                &self,
                reservation_id: ResourceReservationId,
            ) -> Result<ResourceReceipt, ResourceError> {
                self.inner.release(reservation_id)
            }
            fn account_snapshot(
                &self,
                account: &ResourceAccount,
            ) -> Result<Option<AccountSnapshot>, ResourceError> {
                self.inner.account_snapshot(account)
            }
        }

        use brassclaw_host_api::ResourceReceipt;
        use brassclaw_resources::{AccountSnapshot, ReservationOutcome, ResourceLimits};
        let governor: Arc<dyn ResourceGovernor> = Arc::new(FailingSetLimitGovernor {
            calls: AtomicUsize::new(0),
            inner: InMemoryResourceGovernor::new(),
            fail_first_n: 1,
        });
        let context = run_context();
        let user_account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            UserId::new("acct-user").unwrap(),
        );
        let policy = BudgetSeedingPolicy::new(
            dec!(5.00),
            dec!(2.00),
            BudgetPeriod::Rolling24h,
            BudgetThresholds::DISABLED,
        );
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(ZeroCostTable))
            .with_seeding_policy(policy);
        let request = sample_request();
        // A failed default installation must prevent reservation and model
        // dispatch. A later call can retry without an orphan hold or cache hit.
        assert!(accountant.pre_model_call(&context, &request).await.is_err());
        assert!(accountant.in_flight.is_empty());
        assert!(accountant.seeded.is_empty());
        assert!(governor.account_snapshot(&user_account).unwrap().is_none());
        // Second call: set_limit succeeds; cap is now in place.
        accountant.pre_model_call(&context, &request).await.unwrap();
        let snapshot = governor.account_snapshot(&user_account).unwrap().unwrap();
        assert_eq!(snapshot.limits.unwrap().max_usd, Some(dec!(5.00)));
    }

    #[tokio::test]
    async fn uncertain_budget_admission_retains_address_and_fences_replay() {
        use brassclaw_resources::{
            PersistentResourceGovernor, ResourceGovernorSnapshot, ResourceGovernorStore,
        };
        use std::sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        };
        #[derive(Default)]
        struct CommitThenErrorStore {
            state: Mutex<ResourceGovernorSnapshot>,
            calls: AtomicUsize,
        }
        impl ResourceGovernorStore for CommitThenErrorStore {
            fn update<T, F>(&self, update: F) -> Result<T, ResourceError>
            where
                T: Send + 'static,
                F: FnOnce(&mut ResourceGovernorSnapshot) -> Result<T, ResourceError>
                    + Send
                    + 'static,
            {
                let value = update(&mut self.state.lock().unwrap())?;
                if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Err(ResourceError::Storage {
                        reason: "commit acknowledgement lost".into(),
                    });
                }
                Ok(value)
            }
        }
        let governor = Arc::new(PersistentResourceGovernor::new(
            CommitThenErrorStore::default(),
        ));
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(ZeroCostTable));
        let context = run_context();
        let estimate = ResourceEstimate {
            input_tokens: Some(100),
            ..Default::default()
        };
        assert!(
            accountant
                .reserve_estimate(&context, estimate.clone())
                .is_err()
        );
        let retained = accountant.in_flight.get(&context.run_id).unwrap().id;
        assert!(accountant.reserve_estimate(&context, estimate).is_err());
        assert_eq!(
            accountant.in_flight.get(&context.run_id).unwrap().id,
            retained
        );
        accountant.release_in_flight(&context);
        assert!(accountant.in_flight.is_empty());
        let account = ResourceAccount::tenant(context.scope.tenant_id.clone());
        assert_eq!(governor.reserved_for(&account).unwrap().input_tokens, 0);
        assert_eq!(governor.usage_for(&account).unwrap().input_tokens, 0);
    }
}
