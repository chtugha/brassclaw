//! Real claimed PostgreSQL task host plus actual Auth/SecretStore metadata.
//! This validates binding/fencing, not an activated native credential workflow.
use super::*;
use brassclaw_auth::{AuthProductError, CredentialAccount, CredentialAccountId};
use std::sync::atomic::{AtomicBool, Ordering};

struct GatedAccounts {
    inner: Arc<dyn CredentialAccountRecordSource>,
    armed: AtomicBool,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait]
impl CredentialAccountRecordSource for GatedAccounts {
    async fn account_record(
        &self,
        scope: &AuthProductScope,
        account: CredentialAccountId,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        if self.armed.load(Ordering::Acquire) {
            self.started.notify_one();
            self.release.notified().await;
        }
        self.inner.account_record(scope, account).await
    }
    async fn requirement_selection(
        &self,
        scope: &AuthProductScope,
        requirement: &RuntimeCredentialAccountRequirement,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSelection>, AuthProductError> {
        self.inner.requirement_selection(scope, requirement).await
    }
    async fn accounts_for_owner(
        &self,
        scope: &AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        self.inner.accounts_for_owner(scope).await
    }
}

async fn resolve(
    resolver: &dyn RuntimeCredentialAccountResolver,
    scope: &ResourceScope,
    requirement: &RuntimeCredentialAccountRequirement,
) -> Result<RuntimeCredentialAccessSecret, CredentialStageError> {
    resolver
        .resolve_access_secret(request(scope, requirement))
        .await
}

fn request<'a>(
    scope: &'a ResourceScope,
    requirement: &'a RuntimeCredentialAccountRequirement,
) -> RuntimeCredentialAccountRequest<'a> {
    let declaration = requirement.requirement();
    let RuntimeCredentialRequirementSource::ProductAuthAccount { provider, .. } =
        &declaration.source
    else {
        panic!("account declaration");
    };
    RuntimeCredentialAccountRequest {
        requirement: Some(requirement),
        scope,
        provider,
        provider_scopes: &declaration.provider_scopes,
        requester_extension: requirement.requester_extension(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bound_account_view_fences_original_claim_scope_late_read_and_weak_lifetime() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let admitted = crate::bootstrap_validation::reserve_child_review_parent(
        rig.pool.clone(),
        "tool-credential-view",
        "no Tool or model dispatch",
    )
    .await
    .unwrap();
    let host = admitted.host.clone();
    assert!(host.claimed_run().is_some());
    let context = host.run_context().expect("actual native chat fixture");
    let scope = ResourceScope {
        tenant_id: context.scope.tenant_id.clone(),
        user_id: context.actor.as_ref().unwrap().user_id.clone(),
        agent_id: context.scope.agent_id.clone(),
        project_id: context.scope.project_id.clone(),
        thread_id: Some(context.thread_id.clone()),
        invocation_id: InvocationId::new(),
    };
    let mut fixture = crate::product_auth_runtime_credentials::captured::tests::Fixture::new();
    fixture.scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
    let account = fixture.account("original-account", true).await;
    fixture.select(&account).await;
    let accounts = Arc::new(GatedAccounts {
        inner: fixture.accounts.clone(),
        armed: AtomicBool::new(false),
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let usage = CapturedAccountUsage::capture(
        accounts.clone(),
        fixture.secrets.as_ref(),
        fixture.scope.clone(),
        fixture.requirement.clone(),
    )
    .await
    .unwrap();
    let reference = fixture.requirement.reference().unwrap();
    let captured = CapturedToolCredentials::Captured(Arc::new(ToolCredentialFamily {
        owner: Some(Arc::new(ToolCredentialSourceOwner::new(
            accounts.clone(),
            fixture.secrets.clone(),
        ))),
        scope: Some(fixture.scope.clone()),
        usages: BTreeMap::from([(reference, usage)]),
        raw_usages: BTreeMap::new(),
    }));
    for field in 0..5 {
        let mut foreign = scope.clone();
        match field {
            0 => foreign.tenant_id = brassclaw_host_api::TenantId::new("foreign").unwrap(),
            1 => foreign.user_id = brassclaw_host_api::UserId::new("foreign").unwrap(),
            2 => foreign.agent_id = None,
            3 => foreign.project_id = Some(brassclaw_host_api::ProjectId::new("foreign").unwrap()),
            _ => foreign.thread_id = None,
        }
        assert!(
            captured.bind_invocation(&host, &foreign).is_err(),
            "foreign scope field {field}"
        );
    }
    let services = captured.bind_invocation(&host, &scope).unwrap();
    let resolver = services.accounts;
    let original = resolve(resolver.as_ref(), &scope, &fixture.requirement)
        .await
        .unwrap();
    assert_eq!(original.handle, account.access_secret.unwrap());
    assert!(
        services
            .guard
            .check_source_use(&request(&scope, &fixture.requirement), &original)
            .is_ok()
    );
    let mut replacement = original.clone();
    replacement.record_reference = "a different encrypted generation".into();
    assert!(
        services
            .guard
            .check_source_use(&request(&scope, &fixture.requirement), &replacement)
            .is_err()
    );
    let mut other_invocation = scope.clone();
    other_invocation.invocation_id = InvocationId::new();
    assert!(
        resolve(resolver.as_ref(), &other_invocation, &fixture.requirement)
            .await
            .is_err()
    );
    assert!(
        fixture
            .secrets
            .leases_for_scope(&scope)
            .await
            .unwrap()
            .is_empty()
    );

    accounts.armed.store(true, Ordering::Release);
    let pending_resolver = resolver.clone();
    let pending_scope = scope.clone();
    let requirement = fixture.requirement.clone();
    let pending = tokio::spawn(async move {
        resolve(pending_resolver.as_ref(), &pending_scope, &requirement).await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        accounts.started.notified(),
    )
    .await
    .unwrap();
    host.fence_dispatch();
    assert!(
        services
            .guard
            .check_source_use(&request(&scope, &fixture.requirement), &original)
            .is_err()
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), pending)
            .await
            .expect("fencing must not wait for the read backend")
            .unwrap()
            .is_err(),
        "a late source read cannot reopen the old attempt"
    );
    accounts.release.notify_one();
    assert!(captured.bind_invocation(&host, &scope).is_err());
    assert!(
        resolve(resolver.as_ref(), &scope, &fixture.requirement)
            .await
            .is_err()
    );
    let weak = Arc::downgrade(&host);
    drop(host);
    drop(admitted);
    assert!(
        weak.upgrade().is_none(),
        "credential views cannot retain a completed task host"
    );
    assert!(
        resolve(resolver.as_ref(), &scope, &fixture.requirement)
            .await
            .is_err()
    );
    assert!(
        fixture
            .secrets
            .leases_for_scope(&scope)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bound_raw_view_retains_original_generation_absence_scope_and_host_fence() {
    use brassclaw_host_api::{NetworkTargetPattern, RuntimeCredentialTarget, SecretHandle};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let admitted = crate::bootstrap_validation::reserve_child_review_parent(
        rig.pool.clone(),
        "raw-source-view",
        "no native Tool dispatch",
    )
    .await
    .unwrap();
    let host = admitted.host.clone();
    let context = host.run_context().expect("actual native chat fixture");
    let scope = ResourceScope {
        tenant_id: context.scope.tenant_id.clone(),
        user_id: context.actor.as_ref().unwrap().user_id.clone(),
        agent_id: context.scope.agent_id.clone(),
        project_id: context.scope.project_id.clone(),
        thread_id: Some(context.thread_id.clone()),
        invocation_id: InvocationId::new(),
    };
    let secrets = Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "raw-source-view-test-encryption-key".into(),
            "raw-source-view",
        )
        .unwrap(),
    );
    let accounts = Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
    let capability = CapabilityId::new("test.raw-source-view").unwrap();
    let handle = SecretHandle::new("raw-original").unwrap();
    let missing = SecretHandle::new("raw-original-missing").unwrap();
    secrets
        .put(
            scope.clone(),
            handle.clone(),
            "original raw test material".into(),
        )
        .await
        .unwrap();
    let declaration = |handle| RuntimeCredentialRequirement {
        handle,
        source: RuntimeCredentialRequirementSource::SecretHandle,
        provider_scopes: vec![],
        audience: NetworkTargetPattern {
            scheme: None,
            host_pattern: "raw.example".into(),
            port: None,
        },
        target: RuntimeCredentialTarget::Header {
            name: "Authorization".into(),
            prefix: Some("Bearer ".into()),
        },
        required: true,
    };
    let original =
        CapturedRawUsage::capture(secrets.as_ref(), &scope, vec![declaration(handle.clone())])
            .await
            .unwrap();
    let absent =
        CapturedRawUsage::capture(secrets.as_ref(), &scope, vec![declaration(missing.clone())])
            .await
            .unwrap();
    let captured = CapturedToolCredentials::Captured(Arc::new(ToolCredentialFamily {
        owner: Some(Arc::new(ToolCredentialSourceOwner::new(
            accounts,
            secrets.clone(),
        ))),
        scope: Some(AuthProductScope::new(scope.clone(), AuthSurface::Api)),
        usages: BTreeMap::new(),
        raw_usages: BTreeMap::from([(
            capability.as_str().to_owned(),
            BTreeMap::from([
                (handle.as_str().to_owned(), original),
                (missing.as_str().to_owned(), absent),
            ]),
        )]),
    }));
    let bound = captured.bind_invocation(&host, &scope).unwrap();
    let request = RuntimeRawSecretRequest {
        scope: &scope,
        capability_id: &capability,
        handle: &handle,
    };
    let original = bound.raw.resolve_source(request).await.unwrap();
    assert!(bound.raw.check_source_use(&request, &original).is_ok());
    secrets
        .put(scope.clone(), handle.clone(), "later raw material".into())
        .await
        .unwrap();
    secrets
        .put(
            scope.clone(),
            missing.clone(),
            "newly configured raw material".into(),
        )
        .await
        .unwrap();
    assert_eq!(bound.raw.resolve_source(request).await.unwrap(), original);
    assert!(
        secrets
            .lease_once_if_record_reference(
                &original.scope,
                &original.handle,
                &original.record_reference
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        bound
            .raw
            .resolve_source(RuntimeRawSecretRequest {
                handle: &missing,
                ..request
            })
            .await
            .unwrap_err(),
        CredentialStageError::AuthRequired
    );
    let mut different = original.clone();
    different.record_reference = secrets
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    assert!(bound.raw.check_source_use(&request, &different).is_err());
    let mut foreign = scope.clone();
    foreign.invocation_id = InvocationId::new();
    assert!(
        bound
            .raw
            .resolve_source(RuntimeRawSecretRequest {
                scope: &foreign,
                ..request
            })
            .await
            .is_err()
    );
    let other = CapabilityId::new("test.other-raw-source").unwrap();
    assert!(
        bound
            .raw
            .resolve_source(RuntimeRawSecretRequest {
                capability_id: &other,
                ..request
            })
            .await
            .is_err()
    );
    assert!(secrets.leases_for_scope(&scope).await.unwrap().is_empty());
    host.fence_dispatch();
    assert!(bound.raw.check_source_use(&request, &original).is_err());
    assert!(bound.raw.resolve_source(request).await.is_err());
    let weak = Arc::downgrade(&host);
    drop(host);
    drop(admitted);
    assert!(weak.upgrade().is_none());
    assert!(bound.raw.resolve_source(request).await.is_err());
}
