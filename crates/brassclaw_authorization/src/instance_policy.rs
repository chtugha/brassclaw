//! Instance-wide tool permission, independent of callers and operation leases.
//!
//! The source is bound to an instance by trusted composition. It accepts only
//! a registered descriptor, never a user/project/run supplied permission key.
//! Technical execution rules remain separate from tool admission.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, Decision, DenyReason, EffectKind, ExecutionContext,
    MountView, NetworkPolicy, Obligation, ResourceCeiling, ResourceEstimate, SecretHandle,
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
    #[error("instance tool policy revision conflict")]
    RevisionConflict,
}

/// One complete effective generation. Publication never merges fields from
/// different edits. The durable settings adapter owns persistence/acknowledge.
#[derive(Debug, Clone)]
pub struct InstanceToolPolicySnapshot {
    pub revision: u64,
    pub tools: HashMap<CapabilityId, InstanceToolRule>,
}

impl InstanceToolPolicySnapshot {
    fn validate(&self) -> Result<(), InstanceToolPolicyError> {
        if self.revision == 0
            || self
                .tools
                .values()
                .any(|rule| rule.revision != self.revision)
        {
            return Err(InstanceToolPolicyError::Invalid);
        }
        Ok(())
    }
}

/// Effective in-process policy. Composition shares one source with every host;
/// publish is a trusted settings operation, never exposed to a Recipe/LLM.
/// Poisoning fails closed rather than restoring an unverified cached policy.
#[derive(Debug)]
pub struct LiveInstanceToolPolicy {
    snapshot: RwLock<InstanceToolPolicySnapshot>,
}

impl LiveInstanceToolPolicy {
    pub fn new(snapshot: InstanceToolPolicySnapshot) -> Result<Self, InstanceToolPolicyError> {
        snapshot.validate()?;
        Ok(Self {
            snapshot: RwLock::new(snapshot),
        })
    }

    pub fn publish(
        &self,
        expected_revision: u64,
        next: InstanceToolPolicySnapshot,
    ) -> Result<(), InstanceToolPolicyError> {
        next.validate()?;
        let mut current = self
            .snapshot
            .write()
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        if current.revision != expected_revision || next.revision <= expected_revision {
            return Err(InstanceToolPolicyError::RevisionConflict);
        }
        *current = next;
        Ok(())
    }
}

#[async_trait]
impl InstanceToolPolicySource for LiveInstanceToolPolicy {
    async fn current_rule(
        &self,
        descriptor: &CapabilityDescriptor,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError> {
        let current = self
            .snapshot
            .read()
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        Ok(current.tools.get(&descriptor.id).cloned())
    }
}

#[async_trait]
pub trait InstanceToolPolicySource: Send + Sync {
    /// Read one coherent current revision, linearized with settings publication.
    /// Missing rules and failed reads fail closed. Never return a stale cached
    /// revision or combine fields from different settings generations.
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
        let rule = match self.source.current_rule(descriptor).await {
            Ok(Some(rule)) => rule,
            Ok(None) => return denied(DenyReason::PolicyDenied),
            Err(_) => return denied(DenyReason::InternalInvariantViolation),
        };
        Self::decide_rule(context, descriptor, estimate, trust, &rule)
    }

    fn decide_rule(
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust: &TrustDecision,
        rule: &InstanceToolRule,
    ) -> Decision {
        if context.validate().is_err() {
            return denied(DenyReason::InternalInvariantViolation);
        }
        if context.trust != trust.effective_trust.class() {
            return denied(DenyReason::PolicyDenied);
        }
        if rule.revision == 0 {
            return denied(DenyReason::InternalInvariantViolation);
        }
        if !rule.enabled {
            return denied(DenyReason::PolicyDenied);
        }
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

    async fn admit_prepared_dispatch(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust: &TrustDecision,
        prepared: &[Obligation],
    ) -> Result<Option<u64>, DenyReason> {
        // Read one revision after awaited obligation preparation. This snapshot
        // is the dispatch admission point. Do not hold a settings lock while a
        // provider/tool is running: a blocked call must not block live settings.
        let rule = self
            .source
            .current_rule(descriptor)
            .await
            .map_err(|_| DenyReason::InternalInvariantViolation)?
            .ok_or(DenyReason::PolicyDenied)?;
        match Self::decide_rule(context, descriptor, estimate, trust, &rule) {
            Decision::Allow { obligations } if obligations.as_slice() == prepared => {
                Ok(Some(rule.revision))
            }
            Decision::Deny { reason } => Err(reason),
            _ => Err(DenyReason::PolicyDenied),
        }
    }
}
