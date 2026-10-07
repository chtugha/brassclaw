//! Live policy keyed by stable Tool UUID. Trusted registration retains every
//! old dispatch identity while tasks may hold its implementation. Aliases and
//! renamed capabilities read one rule, with mapping and policy published in the
//! same generation. This supplies no catalogue approval or Tool registration.

use std::{collections::HashMap, sync::RwLock};

use async_trait::async_trait;
use brassclaw_host_api::{CapabilityDescriptor, CapabilityId};
use uuid::Uuid;

use crate::{InstanceToolPolicyError, InstanceToolPolicySource, InstanceToolRule};

/// Trusted composition supplies the Tool-to-capability mapping from verified
/// registrations. This is not a deserializable Recipe/operator authority object.
/// A callable alias reaches the same registered capability or has its own
/// registered capability ID mapped to the same stable Tool UUID.
#[derive(Debug, Clone)]
pub struct StableToolPolicySnapshot {
    pub revision: u64,
    pub tools: HashMap<Uuid, InstanceToolRule>,
    pub capabilities: HashMap<CapabilityId, Uuid>,
}
impl StableToolPolicySnapshot {
    fn validate(&self) -> Result<(), InstanceToolPolicyError> {
        if self.revision == 0
            || self
                .tools
                .iter()
                .any(|(id, rule)| id.is_nil() || rule.revision != self.revision)
            || self
                .capabilities
                .values()
                .any(|id| !self.tools.contains_key(id))
        {
            return Err(InstanceToolPolicyError::Invalid);
        }
        Ok(())
    }
}

/// Same authorizer/kernel contract as capability-keyed transitional policy;
/// every read resolves the stable identity and its current rule under one lock.
/// No settings lock is held over obligation preparation or Tool execution.
#[derive(Debug)]
pub struct LiveStableToolPolicy {
    snapshot: RwLock<StableToolPolicySnapshot>,
}
impl LiveStableToolPolicy {
    pub fn new(snapshot: StableToolPolicySnapshot) -> Result<Self, InstanceToolPolicyError> {
        snapshot.validate()?;
        Ok(Self {
            snapshot: RwLock::new(snapshot),
        })
    }

    pub fn publish(
        &self,
        expected_revision: u64,
        next: StableToolPolicySnapshot,
    ) -> Result<(), InstanceToolPolicyError> {
        next.validate()?;
        let mut current = self
            .snapshot
            .write()
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        if current.revision != expected_revision || next.revision <= expected_revision {
            return Err(InstanceToolPolicyError::RevisionConflict);
        }
        // A running/suspended task can retain any previously registered
        // implementation. Never remove or repoint its capability to another
        // Tool to regain permission; disable the stable Tool's rule instead.
        // Reclamation needs a separate proven-drained registration lifecycle.
        if current
            .capabilities
            .iter()
            .any(|(capability, id)| next.capabilities.get(capability) != Some(id))
        {
            return Err(InstanceToolPolicyError::Invalid);
        }
        *current = next;
        Ok(())
    }

    /// Trusted binding preparation checks the registered capability's stable
    /// identity against its retained Tool revision. Missing mapping fails closed.
    /// Existing mappings cannot change, so this identity cannot race publication
    /// into another Tool while kernel admission separately reads current rules.
    pub fn tool_identity(
        &self,
        capability: &CapabilityId,
    ) -> Result<Option<Uuid>, InstanceToolPolicyError> {
        let current = self
            .snapshot
            .read()
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        Ok(current.capabilities.get(capability).copied())
    }

    fn rule(
        &self,
        capability: &CapabilityId,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError> {
        let current = self
            .snapshot
            .read()
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        let Some(id) = current.capabilities.get(capability) else {
            return Ok(None);
        };
        Ok(current.tools.get(id).cloned())
    }
}
#[async_trait]
impl InstanceToolPolicySource for LiveStableToolPolicy {
    async fn current_rule(
        &self,
        descriptor: &CapabilityDescriptor,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError> {
        self.rule(&descriptor.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolExecutionRules;
    use brassclaw_host_api::{EffectKind, MountView, NetworkPolicy};

    fn snapshot(revision: u64, enabled: bool) -> StableToolPolicySnapshot {
        StableToolPolicySnapshot {
            revision,
            tools: HashMap::from([(
                Uuid::from_u128(1),
                InstanceToolRule {
                    revision,
                    enabled,
                    execution: ToolExecutionRules {
                        allowed_effects: vec![EffectKind::DispatchCapability],
                        mounts: MountView::default(),
                        network: NetworkPolicy::default(),
                        secrets: vec![],
                        resource_ceiling: None,
                    },
                },
            )]),
            capabilities: HashMap::from([
                (
                    CapabilityId::new("version-one.json").unwrap(),
                    Uuid::from_u128(1),
                ),
                (
                    CapabilityId::new("version-two.json").unwrap(),
                    Uuid::from_u128(1),
                ),
            ]),
        }
    }
    #[test]
    fn aliases_share_live_rules_and_cannot_be_removed_or_reassigned() {
        let policy = LiveStableToolPolicy::new(snapshot(1, true)).unwrap();
        let first = CapabilityId::new("version-one.json").unwrap();
        let second = CapabilityId::new("version-two.json").unwrap();
        policy.publish(1, snapshot(2, false)).unwrap();
        for id in [&first, &second] {
            assert_eq!(policy.tool_identity(id).unwrap(), Some(Uuid::from_u128(1)));
            let rule = policy.rule(id).unwrap().unwrap();
            assert!(!rule.enabled);
            assert_eq!(rule.revision, 2);
        }
        let mut reassigned = snapshot(3, true);
        let rule = reassigned.tools[&Uuid::from_u128(1)].clone();
        reassigned.tools.insert(Uuid::from_u128(2), rule);
        reassigned
            .capabilities
            .insert(first.clone(), Uuid::from_u128(2));
        assert_eq!(
            policy.publish(2, reassigned),
            Err(InstanceToolPolicyError::Invalid)
        );
        let mut removed = snapshot(3, true);
        removed.capabilities.remove(&second);
        assert_eq!(
            policy.publish(2, removed),
            Err(InstanceToolPolicyError::Invalid)
        );
        let mut missing_rule = snapshot(3, true);
        missing_rule.tools.clear();
        assert_eq!(
            policy.publish(2, missing_rule),
            Err(InstanceToolPolicyError::Invalid)
        );
        assert_eq!(
            policy.publish(1, snapshot(3, true)),
            Err(InstanceToolPolicyError::RevisionConflict)
        );
        assert!(!policy.rule(&first).unwrap().unwrap().enabled);
        assert!(
            policy
                .rule(&CapabilityId::new("unknown.json").unwrap())
                .unwrap()
                .is_none()
        );
        let mut added = snapshot(3, false);
        let alias = CapabilityId::new("version-three.json").unwrap();
        added.capabilities.insert(alias.clone(), Uuid::from_u128(1));
        policy.publish(2, added).unwrap();
        assert!(!policy.rule(&alias).unwrap().unwrap().enabled);
        policy.publish(3, snapshot_with_alias(4, alias)).unwrap();
        assert!(policy.rule(&first).unwrap().unwrap().enabled);
    }
    fn snapshot_with_alias(revision: u64, alias: CapabilityId) -> StableToolPolicySnapshot {
        let mut next = snapshot(revision, true);
        next.capabilities.insert(alias, Uuid::from_u128(1));
        next
    }
}
