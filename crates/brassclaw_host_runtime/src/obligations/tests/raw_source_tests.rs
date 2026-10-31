//! Actual raw obligation/conditional SecretStore/cleanup boundary. The source
//! fixture supplies host-only selection metadata, never a dispatch success.
use super::*;
use secrecy::ExposeSecret;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
struct RawSource {
    scope: ResourceScope,
    capability: CapabilityId,
    handle: SecretHandle,
    source: Option<RuntimeCredentialAccessSecret>,
    checks: AtomicUsize,
    fence_at: usize,
}
#[async_trait]
impl RuntimeRawSecretSource for RawSource {
    async fn resolve_source(
        &self,
        request: RuntimeRawSecretRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, CredentialStageError> {
        if request.scope != &self.scope
            || request.capability_id != &self.capability
            || request.handle != &self.handle
        {
            return Err(CredentialStageError::Backend);
        }
        self.source
            .clone()
            .ok_or(CredentialStageError::AuthRequired)
    }
    fn check_source_use(
        &self,
        request: &RuntimeRawSecretRequest<'_>,
        source: &RuntimeCredentialAccessSecret,
    ) -> Result<(), CredentialStageError> {
        let check = self.checks.fetch_add(1, Ordering::AcqRel) + 1;
        if check >= self.fence_at
            || request.scope != &self.scope
            || request.capability_id != &self.capability
            || request.handle != &self.handle
            || self.source.as_ref() != Some(source)
        {
            return Err(CredentialStageError::Backend);
        }
        Ok(())
    }
}

async fn run(original_exists: bool, replace: bool, fence_at: usize) {
    let context = execution_context();
    let capability = capability_id();
    let handle = SecretHandle::new("raw-original-source").unwrap();
    let secrets = Arc::new(InMemorySecretStore::new());
    if original_exists {
        secrets
            .put(
                context.resource_scope.clone(),
                handle.clone(),
                "original raw material".into(),
            )
            .await
            .unwrap();
    }
    let selected = if original_exists {
        Some(
            RuntimeCredentialAccessSecret::capture(
                secrets.as_ref(),
                context.resource_scope.clone(),
                handle.clone(),
                None,
            )
            .await
            .unwrap(),
        )
    } else {
        None
    };
    let owner = Arc::new(RawSource {
        scope: context.resource_scope.clone(),
        capability: capability.clone(),
        handle: handle.clone(),
        source: selected,
        checks: AtomicUsize::new(0),
        fence_at,
    });
    if replace || !original_exists {
        secrets
            .put(
                context.resource_scope.clone(),
                handle.clone(),
                "later raw material".into(),
            )
            .await
            .unwrap();
    }
    let staged = Arc::new(RuntimeSecretInjectionStore::new());
    let handler = BuiltinObligationHandler::new()
        .with_secret_store(secrets.clone())
        .with_secret_injection_store(staged.clone())
        .retain_raw_secret_source(owner);
    let obligations = [Obligation::InjectSecretOnce {
        handle: handle.clone(),
    }];
    let estimate = ResourceEstimate::default();
    let result = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await;
    let leases = secrets
        .leases_for_scope(&context.resource_scope)
        .await
        .unwrap();
    if !original_exists || replace {
        assert!(matches!(
            result,
            Err(CapabilityObligationError::AuthRequired { .. })
        ));
        assert!(
            leases.is_empty(),
            "no lease for later/missing original material"
        );
    } else if fence_at < usize::MAX {
        assert!(result.is_err());
        assert_eq!(leases.len(), 1);
        assert_eq!(
            leases[0].status,
            if fence_at == 3 {
                brassclaw_secrets::SecretLeaseStatus::Revoked
            } else {
                brassclaw_secrets::SecretLeaseStatus::Consumed
            }
        );
    } else {
        result.unwrap();
        assert_eq!(leases.len(), 1);
        assert_eq!(
            leases[0].status,
            brassclaw_secrets::SecretLeaseStatus::Consumed
        );
        assert_eq!(
            staged
                .take(&context.resource_scope, &capability, &handle)
                .unwrap()
                .unwrap()
                .material
                .expose_secret(),
            "original raw material"
        );
    }
    assert!(
        staged
            .take(&context.resource_scope, &capability, &handle)
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn raw_source_uses_original_conditional_generation() {
    run(true, false, usize::MAX).await;
    run(true, true, usize::MAX).await;
    run(false, false, usize::MAX).await;
}

#[tokio::test]
async fn raw_source_fences_after_lease_and_consumption_without_staging() {
    run(true, false, 3).await;
    run(true, false, 4).await;
}

#[test]
fn raw_source_binding_cannot_repair_a_missing_secret_port() {
    let context = execution_context();
    let source = Arc::new(RawSource {
        scope: context.resource_scope,
        capability: capability_id(),
        handle: SecretHandle::new("absent-port").unwrap(),
        source: None,
        checks: AtomicUsize::new(0),
        fence_at: usize::MAX,
    });
    let handler = BuiltinObligationHandler::new().retain_raw_secret_source(source);
    assert!(handler.secret_store.is_none());
    assert!(handler.raw_secret_source.is_none());
}

/// A deterministic concurrent writer at the real AuditBefore await, between
/// the actual prepare preflight and injection. It delegates real audit storage.
struct ReplaceDuringAudit {
    secrets: Arc<InMemorySecretStore>,
    scope: ResourceScope,
    handle: SecretHandle,
    audit: InMemoryAuditSink,
}
#[async_trait]
impl AuditSink for ReplaceDuringAudit {
    async fn emit_audit(&self, record: AuditEnvelope) -> Result<(), brassclaw_events::EventError> {
        self.secrets
            .put(
                self.scope.clone(),
                self.handle.clone(),
                "concurrent replacement".into(),
            )
            .await
            .map_err(|_| brassclaw_events::EventError::Sink {
                reason: "controlled concurrent secret write failed".into(),
            })?;
        self.audit.emit_audit(record).await
    }
}

#[tokio::test]
async fn legacy_preflight_cannot_lease_replacement_and_cleans_earlier_handoffs() {
    for count in [1, 2] {
        let context = execution_context();
        let capability = capability_id();
        let secrets = Arc::new(InMemorySecretStore::new());
        let staged = Arc::new(RuntimeSecretInjectionStore::new());
        let handles = (0..count)
            .map(|index| SecretHandle::new(format!("legacy-preflight-{index}")).unwrap())
            .collect::<Vec<_>>();
        for handle in &handles {
            secrets
                .put(
                    context.resource_scope.clone(),
                    handle.clone(),
                    "original material".into(),
                )
                .await
                .unwrap();
        }
        let audit = Arc::new(ReplaceDuringAudit {
            secrets: secrets.clone(),
            scope: context.resource_scope.clone(),
            handle: handles.last().unwrap().clone(),
            audit: InMemoryAuditSink::new(),
        });
        let handler = BuiltinObligationHandler::new()
            .with_secret_store(secrets.clone())
            .with_secret_injection_store(staged.clone())
            .with_audit_sink(audit.clone());
        assert!(
            handler.raw_secret_source.is_none(),
            "exercise the actual unbound legacy branch"
        );
        let obligations = std::iter::once(Obligation::AuditBefore)
            .chain(
                handles
                    .iter()
                    .cloned()
                    .map(|handle| Obligation::InjectSecretOnce { handle }),
            )
            .collect::<Vec<_>>();
        let estimate = ResourceEstimate::default();
        let result = handler
            .prepare(CapabilityObligationRequest {
                phase: CapabilityObligationPhase::Invoke,
                context: &context,
                capability_id: &capability,
                estimate: &estimate,
                obligations: &obligations,
            })
            .await;
        assert!(matches!(
            result,
            Err(CapabilityObligationError::AuthRequired { .. })
        ));
        assert_eq!(
            audit.audit.records().len(),
            1,
            "replacement occurs at actual prepare await"
        );
        let leases = secrets
            .leases_for_scope(&context.resource_scope)
            .await
            .unwrap();
        assert_eq!(
            leases.len(),
            count - 1,
            "replacement never receives a lease"
        );
        for lease in leases {
            assert_eq!(lease.status, brassclaw_secrets::SecretLeaseStatus::Consumed);
            assert_ne!(&lease.handle, handles.last().unwrap());
        }
        for handle in handles {
            assert!(
                staged
                    .take(&context.resource_scope, &capability, &handle)
                    .unwrap()
                    .is_none(),
                "partial preparation must discard every earlier handoff"
            );
        }
    }
}
