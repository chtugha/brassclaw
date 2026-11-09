//! Host-owned single-case operation. The request contains no executable source,
//! export, input, case selector or caller-declared authorization.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use brassclaw_host_api::{CapabilityId, ResourceScope};

use crate::{
    FirstPartyCapabilityError, FirstPartyCapabilityResult, InvocationServices,
    InvocationServicesError, InvocationServicesResolutionRequest, InvocationServicesResolver,
};

pub use brassclaw_host_api::ORIGINAL_PURE_INVOKE_CAPABILITY_ID as PURE_CASE_CAPABILITY_ID;

/// Already bound to one checked original, immutable case and live executor
/// window by Composition. Implementations retain native receipt custody through
/// uncertainty. This operation grants neither Q1 nor any behavioral verdict.
#[async_trait]
pub trait RuntimePureCasePort: Send + Sync {
    async fn invoke_original(
        &self,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError>;
}

pub(crate) struct PureCaseInvocationView {
    scope: ResourceScope,
    operation: Arc<dyn RuntimePureCasePort>,
    used: AtomicBool,
}
impl PureCaseInvocationView {
    pub(crate) fn new(scope: ResourceScope, operation: Arc<dyn RuntimePureCasePort>) -> Self {
        Self {
            scope,
            operation,
            used: AtomicBool::new(false),
        }
    }
    pub(crate) async fn invoke(
        &self,
        scope: &ResourceScope,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        if scope != &self.scope || self.used.swap(true, Ordering::AcqRel) {
            return Err(crate::first_party_tools::operation_error());
        }
        self.operation.invoke_original().await
    }
}

pub(crate) struct PureCaseBoundServices {
    inner: Arc<dyn InvocationServicesResolver>,
    view: Arc<PureCaseInvocationView>,
}
impl PureCaseBoundServices {
    pub(crate) fn new(
        inner: Arc<dyn InvocationServicesResolver>,
        scope: ResourceScope,
        operation: Arc<dyn RuntimePureCasePort>,
    ) -> Self {
        Self {
            inner,
            view: Arc::new(PureCaseInvocationView::new(scope, operation)),
        }
    }
}
impl InvocationServicesResolver for PureCaseBoundServices {
    fn resolve(
        &self,
        request: InvocationServicesResolutionRequest<'_>,
    ) -> Result<InvocationServices, InvocationServicesError> {
        let selected = request.plan.capability
            == CapabilityId::new(PURE_CASE_CAPABILITY_ID).expect("static capability")
            && request.scope == &self.view.scope;
        // The original resolver and placement/policy checks remain mandatory.
        let mut services = self.inner.resolve(request)?;
        // A nested/retained resolver cannot leak an earlier case's view across
        // this invocation's exact capability/scope boundary.
        services.pure_case = selected.then(|| self.view.clone());
        Ok(services)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_host_api::{
        CapabilitySet, ExecutionContext, ExtensionId, MountView, RuntimeKind, TrustClass, UserId,
    };
    use std::sync::atomic::AtomicUsize;
    // Boundary-only refusal instrument. It never supplies a candidate result or
    // pretends to be a completed native/VM invocation.
    struct RefuseOriginal(AtomicUsize);
    #[async_trait]
    impl RuntimePureCasePort for RefuseOriginal {
        async fn invoke_original(
            &self,
        ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
            self.0.fetch_add(1, Ordering::AcqRel);
            tokio::task::yield_now().await;
            Err(crate::first_party_tools::operation_error())
        }
    }
    fn scope() -> ResourceScope {
        ExecutionContext::local_default(
            UserId::new("original-case-actor").unwrap(),
            ExtensionId::new("original-case-boundary").unwrap(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .unwrap()
        .resource_scope
    }
    #[tokio::test]
    async fn foreign_case_service_scopes_cannot_enter_or_consume_the_original() {
        let original = scope();
        let operation = Arc::new(RefuseOriginal(AtomicUsize::new(0)));
        let view = PureCaseInvocationView::new(original.clone(), operation.clone());
        for field in [
            "invocation",
            "actor",
            "tenant",
            "agent",
            "project",
            "thread",
        ] {
            let mut foreign = original.clone();
            match field {
                "invocation" => foreign.invocation_id = brassclaw_host_api::InvocationId::new(),
                "actor" => foreign.user_id = UserId::new("other-actor").unwrap(),
                "tenant" => {
                    foreign.tenant_id = brassclaw_host_api::TenantId::new("other-tenant").unwrap()
                }
                "agent" => {
                    foreign.agent_id =
                        Some(brassclaw_host_api::AgentId::new("other-agent").unwrap())
                }
                "project" => {
                    foreign.project_id =
                        Some(brassclaw_host_api::ProjectId::new("other-project").unwrap())
                }
                "thread" => {
                    foreign.thread_id =
                        Some(brassclaw_host_api::ThreadId::new("other-thread").unwrap())
                }
                _ => unreachable!(),
            }
            assert!(view.invoke(&foreign).await.is_err());
            assert_eq!(operation.0.load(Ordering::Acquire), 0);
        }
        assert!(view.invoke(&original).await.is_err());
        assert_eq!(
            operation.0.load(Ordering::Acquire),
            1,
            "a rejected foreign scope must not consume the original"
        );
    }
    #[tokio::test]
    async fn shared_case_service_clones_allow_at_most_one_operation_even_on_failure() {
        let original = scope();
        let operation = Arc::new(RefuseOriginal(AtomicUsize::new(0)));
        let view = Arc::new(PureCaseInvocationView::new(
            original.clone(),
            operation.clone(),
        ));
        let clone = view.clone();
        let (first, second) = tokio::join!(view.invoke(&original), clone.invoke(&original));
        assert!(first.is_err());
        assert!(second.is_err());
        assert!(view.invoke(&original).await.is_err());
        assert_eq!(operation.0.load(Ordering::Acquire), 1);
    }
}
