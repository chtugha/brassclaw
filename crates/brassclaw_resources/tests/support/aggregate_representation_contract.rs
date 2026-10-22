use brassclaw_host_api::*;
use brassclaw_resources::*;

fn resource_test_scope(tenant: &str, user: &str, project: Option<&str>) -> ResourceScope {
    ResourceScope {
        tenant_id: TenantId::new(tenant).unwrap(),
        user_id: UserId::new(user).unwrap(),
        agent_id: None,
        project_id: project.map(|p| ProjectId::new(p).unwrap()),
        thread_id: None,
        invocation_id: InvocationId::new(),
    }
}

fn representation_boundary_cases() -> [(ResourceDimension, ResourceEstimate, ResourceEstimate); 8] {
    [
        (
            ResourceDimension::Usd,
            ResourceEstimate {
                usd: Some(rust_decimal::Decimal::MAX),
                ..Default::default()
            },
            ResourceEstimate {
                usd: Some(rust_decimal::Decimal::ONE),
                ..Default::default()
            },
        ),
        (
            ResourceDimension::InputTokens,
            ResourceEstimate {
                input_tokens: Some(u64::MAX),
                ..Default::default()
            },
            ResourceEstimate {
                input_tokens: Some(1),
                ..Default::default()
            },
        ),
        (
            ResourceDimension::OutputTokens,
            ResourceEstimate {
                output_tokens: Some(u64::MAX),
                ..Default::default()
            },
            ResourceEstimate {
                output_tokens: Some(1),
                ..Default::default()
            },
        ),
        (
            ResourceDimension::WallClockMs,
            ResourceEstimate {
                wall_clock_ms: Some(u64::MAX),
                ..Default::default()
            },
            ResourceEstimate {
                wall_clock_ms: Some(1),
                ..Default::default()
            },
        ),
        (
            ResourceDimension::OutputBytes,
            ResourceEstimate {
                output_bytes: Some(u64::MAX),
                ..Default::default()
            },
            ResourceEstimate {
                output_bytes: Some(1),
                ..Default::default()
            },
        ),
        (
            ResourceDimension::NetworkEgressBytes,
            ResourceEstimate {
                network_egress_bytes: Some(u64::MAX),
                ..Default::default()
            },
            ResourceEstimate {
                network_egress_bytes: Some(1),
                ..Default::default()
            },
        ),
        (
            ResourceDimension::ProcessCount,
            ResourceEstimate {
                process_count: Some(u32::MAX),
                ..Default::default()
            },
            ResourceEstimate {
                process_count: Some(1),
                ..Default::default()
            },
        ),
        (
            ResourceDimension::ConcurrencySlots,
            ResourceEstimate {
                concurrency_slots: Some(u32::MAX),
                ..Default::default()
            },
            ResourceEstimate {
                concurrency_slots: Some(1),
                ..Default::default()
            },
        ),
    ]
}

fn boundary_usage(estimate: &ResourceEstimate) -> ResourceUsage {
    ResourceUsage {
        usd: estimate.usd.unwrap_or_default(),
        input_tokens: estimate.input_tokens.unwrap_or_default(),
        output_tokens: estimate.output_tokens.unwrap_or_default(),
        wall_clock_ms: estimate.wall_clock_ms.unwrap_or_default(),
        output_bytes: estimate.output_bytes.unwrap_or_default(),
        network_egress_bytes: estimate.network_egress_bytes.unwrap_or_default(),
        process_count: estimate.process_count.unwrap_or_default(),
    }
}

fn resource_balances(
    governor: &dyn ResourceGovernor,
    accounts: &[ResourceAccount],
) -> Vec<(ResourceTally, ResourceTally)> {
    accounts
        .iter()
        .map(|account| {
            let ledger = governor.account_snapshot(account).unwrap().unwrap().ledger;
            (ledger.spent, ledger.reserved)
        })
        .collect()
}

pub(super) fn aggregate_representation_contract(governor: &dyn ResourceGovernor) {
    for (index, (dimension, maximum, one)) in
        representation_boundary_cases().into_iter().enumerate()
    {
        let scope = resource_test_scope(
            &format!("reserve-boundary-{index}"),
            "user",
            Some("project"),
        );
        let accounts = ResourceAccount::cascade(&scope);
        let first = governor.reserve(scope.clone(), maximum.clone()).unwrap();
        let before = resource_balances(governor, &accounts);
        let rejected_id = ResourceReservationId::new();
        assert!(
            matches!(governor.reserve_with_id(scope.clone(), one.clone(), rejected_id),
            Err(ResourceError::InvalidEstimate { dimension: actual, .. }) if actual == dimension)
        );
        assert_eq!(resource_balances(governor, &accounts), before);
        assert!(matches!(
            governor.release(rejected_id),
            Err(ResourceError::UnknownReservation { .. })
        ));
        if dimension == ResourceDimension::Usd {
            assert!(matches!(
                governor.reserve(
                    scope.clone(),
                    ResourceEstimate {
                        usd: Some(rust_decimal::Decimal::new(1, 28)),
                        ..Default::default()
                    }
                ),
                Err(ResourceError::InvalidEstimate {
                    dimension: ResourceDimension::Usd,
                    ..
                })
            ));
            assert_eq!(resource_balances(governor, &accounts), before);
        }
        governor.release(first.id).unwrap();
        // A normal second hold after the rejected overflow remains exact and
        // independently releasable, rather than inheriting a saturated balance.
        let second = governor.reserve(scope, one.clone()).unwrap();
        governor.release(second.id).unwrap();
        for account in accounts {
            assert!(governor.account_snapshot(&account).unwrap().is_none());
        }
        if dimension == ResourceDimension::ConcurrencySlots {
            continue;
        }

        let scope =
            resource_test_scope(&format!("settle-boundary-{index}"), "user", Some("project"));
        let tenant = ResourceAccount::tenant(scope.tenant_id.clone());
        let accounts = ResourceAccount::cascade(&scope);
        let mut prior = maximum;
        prior.usd = prior
            .usd
            .map(|value| value - rust_decimal::Decimal::from(2));
        prior.input_tokens = prior.input_tokens.map(|value| value - 2);
        prior.output_tokens = prior.output_tokens.map(|value| value - 2);
        prior.wall_clock_ms = prior.wall_clock_ms.map(|value| value - 2);
        prior.output_bytes = prior.output_bytes.map(|value| value - 2);
        prior.network_egress_bytes = prior.network_egress_bytes.map(|value| value - 2);
        prior.process_count = prior.process_count.map(|value| value - 2);
        let first = governor.reserve(scope.clone(), prior.clone()).unwrap();
        governor
            .reconcile(first.id, boundary_usage(&prior))
            .unwrap();
        let pending = governor.reserve(scope.clone(), one.clone()).unwrap();
        // Reset only the ancestor's accounting period. Its settlement fits,
        // while the deeper account overflows: there must be no partial cascade.
        governor
            .set_limit(
                tenant,
                ResourceLimits {
                    period: BudgetPeriod::Rolling24h,
                    ..Default::default()
                },
            )
            .unwrap();
        let before = resource_balances(governor, &accounts);
        let mut actual = boundary_usage(&one);
        actual.usd *= rust_decimal::Decimal::from(3);
        actual.input_tokens *= 3;
        actual.output_tokens *= 3;
        actual.wall_clock_ms *= 3;
        actual.output_bytes *= 3;
        actual.network_egress_bytes *= 3;
        actual.process_count *= 3;
        for _ in 0..2 {
            assert!(matches!(governor.reconcile(pending.id, actual.clone()),
                Err(ResourceError::InvalidEstimate { dimension: actual, .. }) if actual == dimension));
            assert_eq!(resource_balances(governor, &accounts), before);
        }
        // Both persisted spent and the active hold participate in admission.
        let fresh = ResourceReservationId::new();
        assert!(matches!(governor.reserve_with_id(scope, prior, fresh),
            Err(ResourceError::InvalidEstimate { dimension: actual, .. }) if actual == dimension));
        assert!(matches!(
            governor.release(fresh),
            Err(ResourceError::UnknownReservation { .. })
        ));
    }
}
