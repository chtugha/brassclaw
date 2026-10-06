use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolPolicyError, InstanceToolPolicySource, InstanceToolRule,
    ToolExecutionRules,
};
use brassclaw_capabilities::{
    CapabilityHost, CapabilityInvocationError, CapabilityInvocationRequest,
};
use brassclaw_host_api::*;
use serde_json::json;

mod support;
use support::*;

// Mutations/read snapshots use the same lock. The second read models a
// settings publication after initial authorization, before dispatch admission.
struct Policy {
    first: InstanceToolRule,
    next: Mutex<Result<Option<InstanceToolRule>, InstanceToolPolicyError>>,
    reads: AtomicUsize,
}

impl Policy {
    fn new(next: Result<Option<InstanceToolRule>, InstanceToolPolicyError>) -> Self {
        Self {
            first: rule(true, 1),
            next: Mutex::new(next),
            reads: AtomicUsize::new(0),
        }
    }
}

fn rule(enabled: bool, revision: u64) -> InstanceToolRule {
    InstanceToolRule {
        enabled,
        revision,
        execution: ToolExecutionRules {
            allowed_effects: vec![EffectKind::DispatchCapability],
            mounts: MountView::default(),
            network: NetworkPolicy::default(),
            secrets: Vec::new(),
            resource_ceiling: None,
        },
    }
}

#[async_trait]
impl InstanceToolPolicySource for Policy {
    async fn current_rule(
        &self,
        _: &CapabilityDescriptor,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError> {
        if self.reads.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(Some(self.first.clone()))
        } else {
            self.next.lock().expect("policy lock").clone()
        }
    }
}

fn request() -> CapabilityInvocationRequest {
    CapabilityInvocationRequest {
        context: execution_context(CapabilitySet::default()),
        capability_id: capability_id(),
        estimate: ResourceEstimate::default(),
        input: json!({"message": "instance dispatch"}),
        trust_decision: trust_decision(),
    }
}

#[tokio::test]
async fn globally_allowed_tool_dispatches_without_grants_and_reports_current_revision() {
    let registry = registry_with_echo_capability();
    let dispatcher = RecordingDispatcher::default();
    let policy = Arc::new(Policy::new(Ok(Some(rule(true, 2)))));
    let authorizer = InstanceToolAuthorizer::new(policy.clone());
    let result = CapabilityHost::new(&registry, &dispatcher, &authorizer)
        .invoke_json(request())
        .await
        .unwrap();
    assert_eq!(result.policy_revision, Some(2));
    assert!(dispatcher.has_request());
    assert_eq!(policy.reads.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn block_missing_rule_or_policy_failure_between_prepare_and_admission_prevents_effect() {
    for next in [
        Ok(Some(rule(false, 2))),
        Ok(None),
        Err(InstanceToolPolicyError::Unavailable),
    ] {
        let registry = registry_with_echo_capability();
        let dispatcher = RecordingDispatcher::default();
        let authorizer = InstanceToolAuthorizer::new(Arc::new(Policy::new(next)));
        let error = CapabilityHost::new(&registry, &dispatcher, &authorizer)
            .invoke_json(request())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            CapabilityInvocationError::AuthorizationDenied { .. }
        ));
        assert!(!dispatcher.has_request());
    }
}

#[tokio::test]
async fn changed_technical_limits_require_new_preparation_before_effect() {
    let registry = registry_with_echo_capability();
    let dispatcher = RecordingDispatcher::default();
    let mut next = rule(true, 2);
    next.execution.resource_ceiling = Some(ResourceCeiling {
        max_output_bytes: Some(1024),
        max_usd: None,
        max_input_tokens: None,
        max_output_tokens: None,
        max_wall_clock_ms: None,
        sandbox: None,
    });
    let authorizer = InstanceToolAuthorizer::new(Arc::new(Policy::new(Ok(Some(next)))));
    let error = CapabilityHost::new(&registry, &dispatcher, &authorizer)
        .invoke_json(request())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        CapabilityInvocationError::AuthorizationDenied {
            reason: DenyReason::PolicyDenied,
            ..
        }
    ));
    assert!(!dispatcher.has_request());
}

#[tokio::test]
async fn settings_change_during_preparation_aborts_without_dispatch_or_replay() {
    use brassclaw_authorization::{InstanceToolPolicySnapshot, LiveInstanceToolPolicy};
    use brassclaw_capabilities::{
        CapabilityObligationAbortRequest, CapabilityObligationError, CapabilityObligationHandler,
        CapabilityObligationRequest,
    };
    use std::collections::HashMap;
    use tokio::sync::Notify;

    struct WaitingPreparation {
        entered: Notify,
        release: Notify,
        aborted: AtomicUsize,
    }
    #[async_trait]
    impl CapabilityObligationHandler for WaitingPreparation {
        async fn satisfy(
            &self,
            _: CapabilityObligationRequest<'_>,
        ) -> Result<(), CapabilityObligationError> {
            self.entered.notify_one();
            self.release.notified().await;
            Ok(())
        }
        async fn abort(
            &self,
            _: CapabilityObligationAbortRequest<'_>,
        ) -> Result<(), CapabilityObligationError> {
            self.aborted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    let registry = registry_with_echo_capability();
    let dispatcher = RecordingDispatcher::default();
    let mut initial = rule(true, 1);
    initial.execution.network.deny_private_ip_ranges = true;
    let source = Arc::new(
        LiveInstanceToolPolicy::new(InstanceToolPolicySnapshot {
            revision: 1,
            tools: HashMap::from([(capability_id(), initial.clone())]),
        })
        .unwrap(),
    );
    let authorizer = InstanceToolAuthorizer::new(source.clone());
    let handler = WaitingPreparation {
        entered: Notify::new(),
        release: Notify::new(),
        aborted: AtomicUsize::new(0),
    };
    let host =
        CapabilityHost::new(&registry, &dispatcher, &authorizer).with_obligation_handler(&handler);
    let update = async {
        handler.entered.notified().await;
        initial.enabled = false;
        initial.revision = 2;
        source
            .publish(
                1,
                InstanceToolPolicySnapshot {
                    revision: 2,
                    tools: HashMap::from([(capability_id(), initial)]),
                },
            )
            .unwrap();
        assert_eq!(
            source.publish(
                1,
                InstanceToolPolicySnapshot {
                    revision: 3,
                    tools: HashMap::new(),
                }
            ),
            Err(InstanceToolPolicyError::RevisionConflict)
        );
        handler.release.notify_one();
    };
    let (result, ()) = tokio::join!(host.invoke_json(request()), update);
    assert!(matches!(
        result,
        Err(CapabilityInvocationError::AuthorizationDenied {
            reason: DenyReason::PolicyDenied,
            ..
        })
    ));
    assert!(!dispatcher.has_request());
    assert_eq!(handler.aborted.load(Ordering::SeqCst), 1);
}
