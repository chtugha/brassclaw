//! Instance-wide tool permission, independent of callers and operation leases.
//!
//! The source is bound to an instance by trusted composition. It accepts only
//! a registered descriptor, never a user/project/run supplied permission key.
//! Technical execution rules remain separate from tool admission.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{
    CapabilityDescriptor, Decision, DenyReason, EffectKind, ExecutionContext, MountView,
    NetworkPolicy, Obligation, ResourceCeiling, ResourceEstimate, SecretHandle,
};
use brassclaw_trust::TrustDecision;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::TrustAwareCapabilityDispatchAuthorizer;

/// Technical constraints, with no grantee, expiry or invocation allowance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolExecutionRules {
    pub allowed_effects: Vec<EffectKind>,
    pub mounts: MountView,
    pub network: NetworkPolicy,
    pub secrets: Vec<SecretHandle>,
    pub resource_ceiling: Option<ResourceCeiling>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceToolRule {
    pub enabled: bool,
    pub revision: u64,
    pub execution: ToolExecutionRules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InstanceToolPolicyError {
    #[error("instance tool policy unavailable")]
    Unavailable,
    #[error("instance tool policy invalid")]
    Invalid,
}

#[async_trait]
pub trait InstanceToolPolicySource: Send + Sync {
    /// Read current settings. Missing rules and failed reads fail closed.
    async fn current_rule(
        &self,
        descriptor: &CapabilityDescriptor,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError>;
}

pub struct InstanceToolAuthorizer {
    source: Arc<dyn InstanceToolPolicySource>,
}

impl InstanceToolAuthorizer {
    pub fn new(source: Arc<dyn InstanceToolPolicySource>) -> Self {
        Self { source }
    }

    async fn decide(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust: &TrustDecision,
    ) -> Decision {
        if context.validate().is_err() {
            return denied(DenyReason::InternalInvariantViolation);
        }
        if context.trust != trust.effective_trust.class() {
            return denied(DenyReason::PolicyDenied);
        }
        let rule = match self.source.current_rule(descriptor).await {
            Ok(Some(rule)) if rule.enabled => rule,
            Ok(_) => return denied(DenyReason::PolicyDenied),
            Err(_) => return denied(DenyReason::InternalInvariantViolation),
        };
        let ceiling = crate::intersect_resource_ceilings(
            rule.execution.resource_ceiling.as_ref(),
            trust.authority_ceiling.max_resource_ceiling.as_ref(),
        );
        if !crate::effects_are_covered(&descriptor.effects, &rule.execution.allowed_effects)
            || !crate::effects_are_covered(
                &descriptor.effects,
                &trust.authority_ceiling.allowed_effects,
            )
            || !crate::resource_estimate_is_covered(estimate, ceiling.as_ref())
        {
            return denied(DenyReason::PolicyDenied);
        }
        match crate::obligations_for_rules(descriptor, &rule.execution, ceiling) {
            Some(obligations) => Decision::Allow { obligations },
            None => denied(DenyReason::PolicyDenied),
        }
    }
}

fn denied(reason: DenyReason) -> Decision {
    Decision::Deny { reason }
}

#[async_trait]
impl TrustAwareCapabilityDispatchAuthorizer for InstanceToolAuthorizer {
    fn supports_operation_approval(&self) -> bool {
        false
    }

    async fn authorize_dispatch_with_trust(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust: &TrustDecision,
    ) -> Decision {
        self.decide(context, descriptor, estimate, trust).await
    }

    async fn authorize_spawn_with_trust(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust: &TrustDecision,
    ) -> Decision {
        self.decide(
            context,
            &crate::spawn_descriptor(descriptor),
            estimate,
            trust,
        )
        .await
    }

    async fn validate_prepared_dispatch(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust: &TrustDecision,
        prepared: &[Obligation],
    ) -> bool {
        // Read again after awaited obligation preparation. Revocation denies
        // dispatch; changed technical rules require fresh preparation rather
        // than executing with obsolete mounts/network/secret/resource terms.
        matches!(
            self.decide(context, descriptor, estimate, trust).await,
            Decision::Allow { obligations } if obligations.as_slice() == prepared
        )
    }
}
