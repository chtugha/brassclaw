//! Deterministic adaptive heap-budget calculation. Platform measurement and
//! reclamation belong to composition; this module never probes the OS or polls.
//! Parameters are supplied from validated settings/baseline, not fixed RAM
//! percentages. Additional capacity excludes the live Monty heap already used.

use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MontyMemoryPressure {
    Normal,
    Elevated,
    Critical,
}

#[derive(Debug, Clone, Copy)]
pub struct MontyMemorySample {
    /// Capacity available in addition to the already allocated Monty heap,
    /// after host/container limits have been accounted for by the adapter.
    pub additional_capacity_bytes: u64,
    pub pressure: MontyMemoryPressure,
    pub measured_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MontyHeapBudgetConfig {
    pub reserve_bytes: u64,
    pub manual_ceiling_bytes: Option<u64>,
    pub growth_step_bytes: u64,
    pub growth_headroom_bytes: u64,
    pub max_sample_age: Duration,
    /// Finite cold-boot fallback if no reliable measurement exists.
    pub fallback_bytes: u64,
}

impl MontyHeapBudgetConfig {
    fn validate(self) -> Result<(), MontyHeapBudgetError> {
        if self.fallback_bytes == 0
            || self.fallback_bytes == u64::MAX
            || self.growth_step_bytes == 0
            || self.growth_headroom_bytes == 0
            || self.max_sample_age.is_zero()
            || matches!(self.manual_ceiling_bytes, Some(0 | u64::MAX))
        {
            return Err(MontyHeapBudgetError::InvalidSettings);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MontyHeapBudgetError {
    #[error("invalid Monty heap budget settings")]
    InvalidSettings,
    #[error("manual Monty heap ceiling is below live heap")]
    UnsafeManualReduction,
    #[error("live Monty heap exceeds effective budget")]
    AccountingInvariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MontyHeapAdjustmentReason {
    CapacityGrowth,
    CapacityReduction,
    Pressure,
    AwaitingReclamation,
    MeasurementUnavailable,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MontyHeapBudgetDecision {
    pub target_bytes: u64,
    /// Proposed logical limit; only worker acknowledgement can make it effective.
    pub proposed_bytes: u64,
    pub effective_bytes: u64,
    pub pending_reduction: bool,
    pub backpressure: bool,
    pub reason: MontyHeapAdjustmentReason,
}

#[derive(Debug)]
pub struct AdaptiveMontyHeapBudget {
    config: MontyHeapBudgetConfig,
    effective_bytes: u64,
}

impl AdaptiveMontyHeapBudget {
    pub fn new(config: MontyHeapBudgetConfig) -> Result<Self, MontyHeapBudgetError> {
        config.validate()?;
        let effective_bytes = config
            .fallback_bytes
            .min(config.manual_ceiling_bytes.unwrap_or(u64::MAX));
        Ok(Self {
            config,
            effective_bytes,
        })
    }

    /// Settings changes do not overwrite effective budget or any heap counter.
    /// An operator-requested cap below the live heap is rejected atomically.
    pub fn update_config(
        &mut self,
        next: MontyHeapBudgetConfig,
        live_heap_bytes: u64,
    ) -> Result<(), MontyHeapBudgetError> {
        next.validate()?;
        if next
            .manual_ceiling_bytes
            .is_some_and(|cap| cap < live_heap_bytes)
        {
            return Err(MontyHeapBudgetError::UnsafeManualReduction);
        }
        if live_heap_bytes > self.effective_bytes {
            return Err(MontyHeapBudgetError::AccountingInvariant);
        }
        self.config = next;
        Ok(())
    }

    /// Record a real worker acknowledgement, including the observed live heap.
    /// A rejected publication or dropped waiter must never call this method.
    /// Composition serializes publications; this calculator owns no VM counters.
    pub fn acknowledge(
        &mut self,
        effective_bytes: u64,
        live_heap_bytes: u64,
    ) -> Result<(), MontyHeapBudgetError> {
        if effective_bytes == 0 || effective_bytes == u64::MAX {
            return Err(MontyHeapBudgetError::InvalidSettings);
        }
        if live_heap_bytes > effective_bytes {
            return Err(MontyHeapBudgetError::AccountingInvariant);
        }
        self.effective_bytes = effective_bytes;
        Ok(())
    }

    pub fn evaluate(
        &self,
        live_heap_bytes: u64,
        sample: Option<MontyMemorySample>,
        now: Instant,
    ) -> Result<MontyHeapBudgetDecision, MontyHeapBudgetError> {
        if live_heap_bytes > self.effective_bytes {
            return Err(MontyHeapBudgetError::AccountingInvariant);
        }
        let sample = sample.filter(|s| {
            now.checked_duration_since(s.measured_at)
                .is_some_and(|age| age <= self.config.max_sample_age)
        });
        let target = sample.and_then(|s| {
            live_heap_bytes
                .checked_add(s.additional_capacity_bytes)
                .filter(|total| *total != u64::MAX)
                .map(|total_capacity| {
                    let capacity = total_capacity.saturating_sub(self.config.reserve_bytes);
                    let capped = capacity.min(self.config.manual_ceiling_bytes.unwrap_or(u64::MAX));
                    let target = if s.pressure == MontyMemoryPressure::Critical {
                        capped.min(live_heap_bytes)
                    } else {
                        capped
                    };
                    (target, s.pressure)
                })
        });
        let Some((target, pressure)) = target else {
            // No growth from absent/stale/future/overflowing measurement. Keep
            // the last finite limit, and stop admitting new tasks until fresh
            // capacity is known. Cold boot uses the explicit finite fallback.
            return Ok(MontyHeapBudgetDecision {
                target_bytes: self.effective_bytes,
                proposed_bytes: self.effective_bytes,
                effective_bytes: self.effective_bytes,
                pending_reduction: false,
                backpressure: true,
                reason: MontyHeapAdjustmentReason::MeasurementUnavailable,
            });
        };
        if target < live_heap_bytes || target == 0 {
            return Ok(MontyHeapBudgetDecision {
                target_bytes: target,
                proposed_bytes: self.effective_bytes,
                effective_bytes: self.effective_bytes,
                pending_reduction: true,
                backpressure: true,
                reason: MontyHeapAdjustmentReason::AwaitingReclamation,
            });
        }
        let previous = self.effective_bytes;
        let mut proposed = previous;
        let reason = if pressure != MontyMemoryPressure::Normal {
            // Pressure relief clamps usable headroom immediately. Rate limiting
            // cannot authorize consumption of newly unavailable host capacity.
            proposed = previous.min(target);
            MontyHeapAdjustmentReason::Pressure
        } else if target < self.effective_bytes {
            proposed = target;
            MontyHeapAdjustmentReason::CapacityReduction
        } else if self.effective_bytes.saturating_sub(live_heap_bytes)
            <= self.config.growth_headroom_bytes
        {
            proposed = target.min(previous.saturating_add(self.config.growth_step_bytes));
            if proposed > previous {
                MontyHeapAdjustmentReason::CapacityGrowth
            } else {
                MontyHeapAdjustmentReason::Unchanged
            }
        } else {
            MontyHeapAdjustmentReason::Unchanged
        };
        Ok(MontyHeapBudgetDecision {
            target_bytes: target,
            proposed_bytes: proposed,
            effective_bytes: self.effective_bytes,
            pending_reduction: false,
            backpressure: pressure != MontyMemoryPressure::Normal || target == 0,
            reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> MontyHeapBudgetConfig {
        MontyHeapBudgetConfig {
            reserve_bytes: 100,
            manual_ceiling_bytes: None,
            growth_step_bytes: 100,
            growth_headroom_bytes: 20,
            max_sample_age: Duration::from_secs(5),
            fallback_bytes: 200,
        }
    }
    fn sample(now: Instant, additional_capacity_bytes: u64) -> MontyMemorySample {
        MontyMemorySample {
            measured_at: now,
            additional_capacity_bytes,
            pressure: MontyMemoryPressure::Normal,
        }
    }
    #[test]
    fn grows_on_demand_and_does_not_subtract_own_heap_twice() {
        let now = Instant::now();
        let budget = AdaptiveMontyHeapBudget::new(config()).unwrap();
        let unchanged = budget.evaluate(50, Some(sample(now, 1000)), now).unwrap();
        assert_eq!(unchanged.target_bytes, 950);
        assert_eq!(unchanged.effective_bytes, 200);
        let grow = budget.evaluate(190, Some(sample(now, 1000)), now).unwrap();
        assert_eq!(grow.target_bytes, 1090);
        assert_eq!(grow.proposed_bytes, 300);
        assert_eq!(grow.effective_bytes, 200);
    }
    #[test]
    fn publication_is_only_a_proposal_until_actual_worker_acknowledgement() {
        let now = Instant::now();
        let mut budget = AdaptiveMontyHeapBudget::new(config()).unwrap();
        let proposed = budget.evaluate(190, Some(sample(now, 1000)), now).unwrap();
        assert_eq!(proposed.proposed_bytes, 300);
        assert_eq!(proposed.effective_bytes, 200);
        // Rejection, a lost acknowledgement or a dropped publication waiter
        // cannot permit an additional growth step from unacknowledged capacity.
        assert_eq!(
            budget.evaluate(190, Some(sample(now, 1000)), now).unwrap(),
            proposed
        );
        assert_eq!(
            budget.acknowledge(180, 190),
            Err(MontyHeapBudgetError::AccountingInvariant)
        );
        assert_eq!(
            budget.evaluate(190, Some(sample(now, 1000)), now).unwrap(),
            proposed
        );
        budget.acknowledge(300, 190).unwrap();
        let confirmed = budget.evaluate(190, Some(sample(now, 1000)), now).unwrap();
        assert_eq!(confirmed.effective_bytes, 300);
        assert_eq!(confirmed.proposed_bytes, 300);
    }
    #[test]
    fn safe_manual_ceiling_does_not_pretend_allocator_reduction_was_applied() {
        let now = Instant::now();
        let mut budget = AdaptiveMontyHeapBudget::new(config()).unwrap();
        let mut next = config();
        next.manual_ceiling_bytes = Some(150);
        budget.update_config(next, 100).unwrap();
        let proposed = budget.evaluate(100, Some(sample(now, 1000)), now).unwrap();
        assert_eq!(proposed.target_bytes, 150);
        assert_eq!(proposed.proposed_bytes, 150);
        assert_eq!(proposed.effective_bytes, 200);
        budget.acknowledge(150, 100).unwrap();
        assert_eq!(
            budget
                .evaluate(100, Some(sample(now, 1000)), now)
                .unwrap()
                .effective_bytes,
            150
        );
    }
    #[test]
    fn stale_future_and_overflowing_measurement_never_grow_budget() {
        let now = Instant::now();
        let budget = AdaptiveMontyHeapBudget::new(config()).unwrap();
        for measurement in [
            None,
            Some(sample(now - Duration::from_secs(6), 1000)),
            Some(sample(now + Duration::from_secs(1), 1000)),
            Some(sample(now, u64::MAX)),
        ] {
            let decision = budget.evaluate(190, measurement, now).unwrap();
            assert_eq!(decision.effective_bytes, 200);
            assert!(decision.backpressure);
            assert_eq!(
                decision.reason,
                MontyHeapAdjustmentReason::MeasurementUnavailable
            );
        }
    }
    #[test]
    fn unbounded_sentinels_never_become_an_effective_or_proposed_heap_limit() {
        let mut settings = config();
        settings.fallback_bytes = u64::MAX;
        assert!(matches!(
            AdaptiveMontyHeapBudget::new(settings),
            Err(MontyHeapBudgetError::InvalidSettings)
        ));
        settings = config();
        settings.manual_ceiling_bytes = Some(u64::MAX);
        assert!(matches!(
            AdaptiveMontyHeapBudget::new(settings),
            Err(MontyHeapBudgetError::InvalidSettings)
        ));
        let now = Instant::now();
        let budget = AdaptiveMontyHeapBudget::new(config()).unwrap();
        let decision = budget
            .evaluate(0, Some(sample(now, u64::MAX)), now)
            .unwrap();
        assert!(decision.backpressure);
        assert_eq!(
            decision.reason,
            MontyHeapAdjustmentReason::MeasurementUnavailable
        );
        assert_eq!(decision.proposed_bytes, 200);
    }
    #[test]
    fn rejects_manual_reduction_and_keeps_previous_configuration() {
        let now = Instant::now();
        let mut budget = AdaptiveMontyHeapBudget::new(config()).unwrap();
        let mut next = config();
        next.manual_ceiling_bytes = Some(50);
        assert_eq!(
            budget.update_config(next, 100),
            Err(MontyHeapBudgetError::UnsafeManualReduction)
        );
        assert_eq!(
            budget
                .evaluate(190, Some(sample(now, 1000)), now)
                .unwrap()
                .proposed_bytes,
            300
        );
    }
    #[test]
    fn insufficient_reserve_keeps_live_heap_and_marks_reduction_pending() {
        let now = Instant::now();
        let budget = AdaptiveMontyHeapBudget::new(config()).unwrap();
        let decision = budget.evaluate(150, Some(sample(now, 25)), now).unwrap();
        assert_eq!(decision.target_bytes, 75);
        assert_eq!(decision.effective_bytes, 200);
        assert!(decision.pending_reduction);
        assert!(decision.backpressure);
    }

    #[test]
    fn pressure_never_sets_limit_below_live_heap_or_restarts_a_task() {
        let now = Instant::now();
        let budget = AdaptiveMontyHeapBudget::new(config()).unwrap();
        let mut measurement = sample(now, 1000);
        measurement.pressure = MontyMemoryPressure::Critical;
        let decision = budget.evaluate(150, Some(measurement), now).unwrap();
        assert_eq!(decision.proposed_bytes, 150);
        assert_eq!(decision.effective_bytes, 200);
        assert!(decision.backpressure);
        assert_eq!(decision.reason, MontyHeapAdjustmentReason::Pressure);
    }
}
