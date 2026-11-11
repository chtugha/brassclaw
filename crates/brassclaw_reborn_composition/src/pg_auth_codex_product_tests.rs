//! Product preparation through the actual factory, PG Auth and local HTTP
//! issuer. No model completion is invoked; Responses/prefix serving is separate.
use super::*;
use brassclaw_llm::{LlmProvider, ProviderProtocol, ProviderPublication, ProviderRole};
use brassclaw_product_workflow::{LlmConfigService, SetActiveLlmRequest, WebUiAuthenticatedCaller};

const TENANT: &str = "native-device-test";

fn caller() -> WebUiAuthenticatedCaller {
    WebUiAuthenticatedCaller::new(
        TenantId::new(TENANT).unwrap(),
        UserId::new("native-device-operator").unwrap(),
        None,
        None,
    )
}

async fn factory(
    rig: &crate::runtime::test_pg::PgRig,
    home: &std::path::Path,
) -> crate::RebornServices {
    crate::build_reborn_services(
        rig.build_input("native-device-operator", home)
            .with_tenant_id(TENANT)
            .with_runtime_policy(crate::local_dev_runtime_policy().unwrap())
            .with_runtime_process_binding(crate::RebornRuntimeProcessBinding::None),
    )
    .await
    .unwrap()
}

async fn select(
    service: &crate::RebornLlmConfigService,
    provider_id: &str,
    role: ProviderRole,
    model: &str,
) -> Result<
    brassclaw_product_workflow::LlmConfigSnapshot,
    brassclaw_product_workflow::LlmConfigServiceError,
> {
    service
        .set_active(
            caller(),
            selection_request(service, provider_id, role, model).await,
        )
        .await
}

async fn selection_request(
    service: &crate::RebornLlmConfigService,
    provider_id: &str,
    role: ProviderRole,
    model: &str,
) -> SetActiveLlmRequest {
    let snapshot = service.snapshot(caller()).await.unwrap();
    let revision = snapshot
        .providers
        .iter()
        .find(|row| row.id == provider_id)
        .unwrap()
        .revision;
    let name = match role {
        ProviderRole::Kohai => "kohai",
        ProviderRole::Sempai => "sempai",
        ProviderRole::Embedding => "embedding",
    };
    SetActiveLlmRequest {
        mutation_id: uuid::Uuid::new_v4(),
        expected_revision: Some(revision),
        expected_selection_ref: Some(snapshot.role_selection_refs[name].clone()),
        provider_id: provider_id.into(),
        model: Some(model.into()),
        role: Some(role),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_codex_product_use_and_snapshot_share_original_oauth_head_without_dispatch() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let home = tempfile::tempdir().unwrap();
    let services = factory(&rig, home.path()).await;
    let auth = services.product_auth.as_ref().unwrap();
    let owner = auth.native_codex_session_owner().unwrap();
    let issuer = Issuer::start(IssuerMode::Success).await;
    let (_, head, _) = published_login_on_pool(&rig.pool, &owner.auth, &issuer).await;
    let mut calls = issuer.state.calls.load(Ordering::SeqCst);
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        TENANT,
    ));
    assert_eq!(
        repo.get_revision("openai_codex", false)
            .await
            .unwrap()
            .unwrap()
            .definition
            .protocol,
        ProviderProtocol::OpenAiCodex
    );
    let keys = services.llm_key_store();
    let primary = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let sempai = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let handle = Arc::new(brassclaw_llm::LlmReloadHandle::new_with_sempai(
        primary.clone(),
        None,
        Some(sempai.clone()),
    ));
    let session =
        brassclaw_llm::create_session_manager(brassclaw_llm::SessionConfig::default()).await;
    let reload = Arc::new(
        crate::llm_reload::RebornLlmReloadAdapter::new(
            handle,
            session,
            keys.clone(),
            rig.pool.clone(),
            TENANT,
        )
        .with_codex_native_owner(Some(owner)),
    );
    let service = crate::RebornLlmConfigService::new(keys.clone(), rig.pool.clone(), repo, TENANT)
        .with_native_product_auth(auth)
        .with_reload_trigger(reload)
        .with_sempai_swappable(sempai.clone());
    let mut originals = Vec::new();
    for (role, name, model) in [
        (ProviderRole::Kohai, "kohai", "codex-primary"),
        (ProviderRole::Sempai, "sempai", "codex-review"),
    ] {
        let snapshot = select(&service, "openai_codex", role, model).await.unwrap();
        let receipt = snapshot.mutation_receipt.as_ref().unwrap();
        originals.push((receipt.mutation_id, serde_json::to_value(receipt).unwrap()));
        let captured = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), TENANT)
            .capture_selection(
                crate::provider_admin::ProviderSelectionSource::Role(role),
                &keys,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(captured.credential_ref.as_deref(), Some(head.as_str()));
        assert_eq!(captured.definition.protocol, ProviderProtocol::OpenAiCodex);
        let expected = ProviderPublication::Configured {
            selection: captured.revision,
        };
        assert_eq!(snapshot.desired_publications[name], expected);
        assert_eq!(snapshot.runtime_publications[name], expected);
        assert!(
            !snapshot
                .providers
                .iter()
                .find(|row| row.id == "openai_codex")
                .unwrap()
                .api_key_set
        );
    }
    assert_eq!(primary.pinned_provider().1, "codex-primary");
    assert_eq!(sempai.pinned_provider().1, "codex-review");
    let before = service.snapshot(caller()).await.unwrap();
    keys.put("openai_codex", "unused-native-test-api-key".into())
        .await
        .unwrap();
    let after = service.snapshot(caller()).await.unwrap();
    assert!(
        after
            .providers
            .iter()
            .find(|row| row.id == "openai_codex")
            .unwrap()
            .api_key_set
    );
    assert_eq!(after.desired_publications, before.desired_publications);
    assert_eq!(after.runtime_publications, before.runtime_publications);
    for (role, name, model) in [
        (ProviderRole::Kohai, "kohai", "codex-primary-with-api-key"),
        (ProviderRole::Sempai, "sempai", "codex-review-with-api-key"),
    ] {
        let snapshot = select(&service, "openai_codex", role, model).await.unwrap();
        let captured = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), TENANT)
            .capture_selection(
                crate::provider_admin::ProviderSelectionSource::Role(role),
                &keys,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(captured.credential_ref.as_deref(), Some(head.as_str()));
        assert_eq!(
            snapshot.runtime_publications[name],
            ProviderPublication::Configured {
                selection: captured.revision
            }
        );
    }
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), calls);

    // Obtain a second real issuer result before the commit barrier. Publishing
    // it later uses the actual native Account/Head/receipt transaction, without
    // crossing HTTP while a provider or credential lock is held.
    let transport = transport(
        &rig.pool,
        &auth.native_codex_owner().unwrap(),
        &issuer,
        None,
        None,
    )
    .await;
    let original = prepare(&auth.native_codex_owner().unwrap(), &transport, 60)
        .await
        .original;
    let challenge = step(&auth.native_codex_owner().unwrap(), &transport, &original).await;
    wait_until_due(&challenge).await;
    step(&auth.native_codex_owner().unwrap(), &transport, &original).await;
    step(&auth.native_codex_owner().unwrap(), &transport, &original).await;
    calls = issuer.state.calls.load(Ordering::SeqCst);
    let native = auth.native_codex_owner().unwrap();
    let mut current_head = head;
    for remove_head in [false, true] {
        let before = service.snapshot(caller()).await.unwrap();
        let request = selection_request(
            &service,
            "openai_codex",
            ProviderRole::Sempai,
            "codex-stale-preparation",
        )
        .await;
        let mutation_id = request.mutation_id;
        let revision = request.expected_revision.unwrap();
        let mut connection = rig.pool.get().await.unwrap();
        let pid: i32 = connection
            .query_one("SELECT pg_backend_pid()", &[])
            .await
            .unwrap()
            .get(0);
        let tx = connection.transaction().await.unwrap();
        tx.query_one("SELECT id FROM brassclaw_llm_providers WHERE tenant_id=$1 AND id='openai_codex' FOR SHARE", &[&TENANT]).await.unwrap();
        tx.batch_execute("LOCK TABLE brassclaw_config IN SHARE ROW EXCLUSIVE MODE")
            .await
            .unwrap();
        let pending = {
            let service = service.clone();
            tokio::spawn(async move { service.set_active(caller(), request).await })
        };
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while !rig.pool.get().await.unwrap().query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.query LIKE 'LOCK TABLE brassclaw_config IN SHARE ROW EXCLUSIVE MODE%' AND $1=ANY(pg_blocking_pids(a.pid)))",
                &[&pid],
            ).await.unwrap().get::<_, bool>(0) {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("actual Use finished preparation and waits on the native writer's role config lock");
        if remove_head {
            // Actual operator secret deletion, not a forged replacement head.
            assert!(
                native
                    .secret_store
                    .delete(
                        &ResourceScope::system(),
                        &crate::llm_key_store::oauth_handle_for("openai_codex").unwrap(),
                    )
                    .await
                    .unwrap()
            );
        } else {
            // If Use had taken the head row before the config lock, this
            // actual native writer would deadlock on that row. Both writers
            // must follow provider -> config -> head and then recheck identity.
            let (next_head, _) = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                publish_login_in_transaction(
                    &native,
                    &tx,
                    &original,
                    revision,
                    Some(&current_head),
                ),
            )
            .await
            .expect("native writer can publish its head while Use waits for config");
            assert_ne!(next_head, current_head);
            current_head = next_head;
        }
        tx.commit().await.unwrap();
        drop(connection);
        assert!(
            matches!(pending.await.unwrap(), Err(brassclaw_product_workflow::LlmConfigServiceError::Conflict { reason })
            if reason == "Provider or authentication changed during preparation; reload before retrying.")
        );
        assert!(matches!(
            service.provider_mutation(caller(), mutation_id).await,
            Err(brassclaw_product_workflow::LlmConfigServiceError::NotFound)
        ));
        let after = service.snapshot(caller()).await.unwrap();
        assert_eq!(after.role_selection_refs, before.role_selection_refs);
        assert_eq!(after.runtime_publications, before.runtime_publications);
        if !remove_head {
            assert_ne!(after.desired_publications, before.desired_publications);
            for role in [ProviderRole::Kohai, ProviderRole::Sempai] {
                let snapshot = select(&service, "openai_codex", role, "codex-current-head")
                    .await
                    .unwrap();
                let captured = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), TENANT)
                    .capture_selection(
                        crate::provider_admin::ProviderSelectionSource::Role(role),
                        &keys,
                    )
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    captured.credential_ref.as_deref(),
                    Some(current_head.as_str())
                );
                let name = if role == ProviderRole::Kohai {
                    "kohai"
                } else {
                    "sempai"
                };
                assert_eq!(
                    snapshot.runtime_publications[name],
                    ProviderPublication::Configured {
                        selection: captured.revision
                    }
                );
            }
        }
    }
    for (id, receipt) in originals {
        assert_eq!(
            serde_json::to_value(service.provider_mutation(caller(), id).await.unwrap()).unwrap(),
            receipt
        );
    }
    // Preparation never asks the native source for a token/refresh or invokes
    // the model. HTTP above is limited to the two explicit login exchanges.
    for _ in 0..2 {
        service.snapshot(caller()).await.unwrap();
    }
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), calls);
    assert_eq!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
                &[&TENANT]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_codex_product_startup_restores_sempai_without_webui_or_dispatch() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let services = factory(&rig, home.path()).await;
    let auth = services.product_auth.as_ref().unwrap();
    let issuer = Issuer::start(IssuerMode::Success).await;
    let (_, head, _) =
        published_login_on_pool(&rig.pool, &auth.native_codex_owner().unwrap(), &issuer).await;
    let calls = issuer.state.calls.load(Ordering::SeqCst);
    crate::db_config::save_config_changes(
        &rig.pool,
        TENANT,
        &[
            ("llm.sempai.provider_id", Some("openai_codex")),
            ("llm.sempai.model", None),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let selected = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), TENANT)
        .capture_selection(
            crate::provider_admin::ProviderSelectionSource::Role(ProviderRole::Sempai),
            &services.llm_key_store(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.definition.protocol, ProviderProtocol::OpenAiCodex);
    assert!(selected.selection.model.is_none());
    let expected = ProviderPublication::Configured {
        selection: selected.revision.clone(),
    };
    drop(services);
    let runtime = crate::build_reborn_runtime(
        crate::RebornRuntimeInput::from_services(
            rig.build_input("native-device-operator", home.path())
                .with_tenant_id(TENANT)
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_identity(crate::RebornRuntimeIdentity {
            tenant_id: TENANT.into(),
            agent_id: "native-codex-startup".into(),
            source_binding_id: "native-codex-source".into(),
            reply_target_binding_id: "native-codex-reply".into(),
        }),
    )
    .await
    .unwrap();
    let restored = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), TENANT)
        .capture_selection(
            crate::provider_admin::ProviderSelectionSource::Role(ProviderRole::Sempai),
            &runtime.services().llm_key_store(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.credential_ref.as_deref(), Some(head.as_str()));
    assert_eq!(restored.revision, selected.revision);
    let sempai = runtime.sempai_swappable().unwrap();
    assert_eq!(
        sempai.pinned_provider().1,
        selected.definition.default_model
    );
    assert_eq!(sempai.publication_status().effective, expected);
    let retained = sempai.pinned_provider().0;
    let prepared = sempai.prepare_retained_provider().await.unwrap().unwrap();
    assert!(Arc::ptr_eq(&retained, &prepared.provider));
    assert_eq!(prepared.publication, expected);
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), calls);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_codex_product_missing_head_or_owner_fails_before_selection_other_protocol_prepares()
{
    let rig = crate::runtime::test_pg::pg_rig().await;
    let home = tempfile::tempdir().unwrap();
    let services = factory(&rig, home.path()).await;
    let auth = services.product_auth.as_ref().unwrap();
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        TENANT,
    ));
    repo.initialize().await.unwrap();
    let sempai = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let keys = services.llm_key_store();
    let service = crate::RebornLlmConfigService::new(keys, rig.pool.clone(), repo.clone(), TENANT)
        .with_sempai_swappable(sempai.clone());
    let native = service.clone().with_native_product_auth(auth);
    assert!(
        select(
            &native,
            "openai_codex",
            ProviderRole::Sempai,
            "codex-missing-head"
        )
        .await
        .is_err()
    );
    let issuer = Issuer::start(IssuerMode::Success).await;
    published_login_on_pool(&rig.pool, &auth.native_codex_owner().unwrap(), &issuer).await;
    let calls = issuer.state.calls.load(Ordering::SeqCst);
    assert!(
        select(
            &service,
            "openai_codex",
            ProviderRole::Sempai,
            "codex-missing-owner"
        )
        .await
        .is_err()
    );
    assert!(
        service
            .snapshot(caller())
            .await
            .unwrap()
            .sempai_active
            .is_none()
    );
    // Other protocols can prepare with no native Auth source attached.
    for id in ["openai", "anthropic", "gemini"] {
        services
            .llm_key_store()
            .put(id, "native-test-api-key".into())
            .await
            .unwrap();
        let definition = repo
            .get_revision(id, false)
            .await
            .unwrap()
            .unwrap()
            .definition;
        assert_ne!(definition.protocol, ProviderProtocol::OpenAiCodex);
        select(
            &service,
            id,
            ProviderRole::Sempai,
            &definition.default_model,
        )
        .await
        .unwrap();
        assert_eq!(sempai.pinned_provider().1, definition.default_model);
    }
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), calls);
}

#[path = "pg_auth_codex_request_tests.rs"]
mod request_tests;

#[path = "pg_auth_codex_status_tests.rs"]
mod status_tests;
