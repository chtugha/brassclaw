//! Actual factory/PG/native source regression. Setup uses the existing real local
//! Device issuer. Unsupported requests must enter neither fresh-access leasing
//! nor the near-expiry refresh source.
//! PG tripwires make accidental credential effects observable across rollback
//! and stop before any model or external auth send; no fake SessionSource.
use super::*;
use brassclaw_llm::{
    ChatMessage, CompletionRequest, ProviderDispatchPolicy, ToolCall, ToolCompletionRequest,
    ToolDefinition,
};

fn single(messages: Vec<ChatMessage>) -> CompletionRequest {
    let mut request = CompletionRequest::new(messages);
    request.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
    request
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_codex_factory_rejects_profiles_and_options_before_any_auth_effect() {
    for mode in [IssuerMode::Success, IssuerMode::NearExpiry] {
        assert_native_preflight_before_auth(mode).await;
    }
}

async fn assert_native_preflight_before_auth(mode: IssuerMode) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let home = tempfile::tempdir().unwrap();
    let services = factory(&rig, home.path()).await;
    let auth = services.product_auth.as_ref().unwrap();
    let native = auth.native_codex_owner().unwrap();
    let issuer = Issuer::start(mode).await;
    let (_, head, account) = published_login_on_pool(&rig.pool, &native, &issuer).await;

    // Use the actual retained login's API target, so session() cannot merely
    // fail on a mismatched fixture endpoint before reaching credential custody.
    let mut connection = rig.pool.get().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    let registration = crate::provider_codex_credentials::original_registration(
        &tx,
        &native.secret_store,
        TENANT,
        "openai_codex",
        &head,
    )
    .await
    .unwrap();
    let api_base_url = registration.binding.client.api_base_url;
    tx.commit().await.unwrap();
    drop(connection);
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        TENANT,
    ));
    let row = repo
        .get_revision("openai_codex", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.definition.protocol, ProviderProtocol::OpenAiCodex);
    let mut definition = row.definition;
    definition.default_base_url = Some(api_base_url);
    let model = definition.default_model.clone();
    repo.replace_revision(definition, Some(row.revision))
        .await
        .unwrap();
    let sempai = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let service = crate::RebornLlmConfigService::new(
        services.llm_key_store(),
        rig.pool.clone(),
        repo,
        TENANT,
    )
    .with_native_product_auth(auth)
    .with_sempai_swappable(sempai.clone());
    select(&service, "openai_codex", ProviderRole::Sempai, &model)
        .await
        .unwrap();
    let provider = sempai.pinned_provider().0;
    assert!(provider.supports_final_request_guard());
    assert_eq!(provider.active_model_name(), model);
    let remaining = account.access_expires_at.unwrap() - Utc::now();
    assert!(remaining > chrono::Duration::zero());
    if mode == IssuerMode::NearExpiry {
        assert!(remaining < chrono::Duration::seconds(300));
    } else {
        assert!(remaining > chrono::Duration::seconds(300));
    }
    let issuer_calls = issuer.state.calls.load(Ordering::SeqCst);
    let baseline = native
        .read_account(&account.scope, account.id)
        .await
        .unwrap()
        .unwrap()
        .0;

    // A consumed lease is deleted in the same transaction, so row counts alone
    // cannot prove no attempt. nextval survives rollback; the real PG failure
    // then prevents material/issuer/model effects even if the preflight regresses.
    rig.pool.get().await.unwrap().batch_execute(r#"
        CREATE SEQUENCE codex_preflight_lease_attempts;
        CREATE SEQUENCE codex_preflight_refresh_attempts;
        CREATE FUNCTION reject_codex_preflight_lease() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.tenant_id='native-device-test' AND NEW.scope LIKE 'lease-v2:%' THEN
                PERFORM nextval('codex_preflight_lease_attempts');
                RAISE EXCEPTION 'controlled unexpected Codex credential lease';
            END IF;
            RETURN NEW;
        END $$;
        CREATE TRIGGER reject_codex_preflight_lease BEFORE INSERT ON brassclaw_secrets
            FOR EACH ROW EXECUTE FUNCTION reject_codex_preflight_lease();
        CREATE FUNCTION reject_codex_preflight_refresh() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.tenant_id='native-device-test' THEN
                PERFORM nextval('codex_preflight_refresh_attempts');
                RAISE EXCEPTION 'controlled unexpected Codex refresh write';
            END IF;
            RETURN NEW;
        END $$;
        CREATE TRIGGER reject_codex_preflight_refresh BEFORE INSERT OR UPDATE ON brassclaw_product_auth_refreshes
            FOR EACH ROW EXECUTE FUNCTION reject_codex_preflight_refresh();
    "#).await.unwrap();
    let mut unsupported = ChatMessage::assistant("Original content");
    unsupported.reasoning = Some("unsupported original reasoning artifact".into());
    let mut options = single(vec![ChatMessage::user("Original constraints")]);
    options.temperature = Some(0.5);
    for request in [single(vec![unsupported]), options] {
        let error = provider.complete(request).await.unwrap_err();
        assert!(matches!(
            error,
            brassclaw_llm::LlmError::FinalRequestRejected {
                reason: brassclaw_llm::final_request_guard::FinalRequestError::Body,
            }
        ));
    }
    let tool = ToolDefinition {
        name: "read".into(),
        description: "Read".into(),
        parameters: serde_json::json!({"type":"object"}),
    };
    let signed = ChatMessage::assistant_with_tool_calls(
        None,
        vec![ToolCall {
            id: "original-call".into(),
            name: "read".into(),
            arguments: serde_json::json!({}),
            reasoning: None,
            signature: Some("unsupported original signature".into()),
        }],
    );
    let profile =
        ToolCompletionRequest::from_completion_request(single(vec![signed]), vec![tool.clone()]);
    let mut options = ToolCompletionRequest::from_completion_request(
        single(vec![ChatMessage::user("Read")]),
        vec![tool],
    );
    options.tool_choice = Some("required".into());
    for request in [profile, options] {
        let error = provider.complete_with_tools(request).await.unwrap_err();
        assert!(matches!(
            error,
            brassclaw_llm::LlmError::FinalRequestRejected {
                reason: brassclaw_llm::final_request_guard::FinalRequestError::Body,
            }
        ));
    }
    let client = rig.pool.get().await.unwrap();
    for sequence in [
        "codex_preflight_lease_attempts",
        "codex_preflight_refresh_attempts",
    ] {
        let row = client
            .query_one(&format!("SELECT is_called FROM {sequence}"), &[])
            .await
            .unwrap();
        assert!(
            !row.get::<_, bool>(0),
            "unsupported requests must not enter credential custody"
        );
    }
    let refreshes: i64 = client
        .query_one(
            "SELECT count(*) FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
            &[&TENANT],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(refreshes, 0);
    drop(client);
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), issuer_calls);
    assert_eq!(
        native
            .read_account(&account.scope, account.id)
            .await
            .unwrap()
            .unwrap()
            .0,
        baseline
    );

    // Connected sensitivity controls, AFTER the four zero-auth cases. Both
    // supported entry kinds reach the actual source: fresh access attempts its
    // lease; near-expiry access first attempts durable refresh admission. The
    // real PG failures stop before credential publication/egress/model.
    for (index, tools) in [false, true].into_iter().enumerate() {
        let request = single(vec![ChatMessage::user("Supported control")]);
        let error = if tools {
            provider
                .complete_with_tools(ToolCompletionRequest::from_completion_request(
                    request,
                    vec![ToolDefinition {
                        name: "read".into(),
                        description: "Read".into(),
                        parameters: serde_json::json!({"type":"object"}),
                    }],
                ))
                .await
                .unwrap_err()
        } else {
            provider.complete(request).await.unwrap_err()
        };
        if mode == IssuerMode::NearExpiry {
            assert!(matches!(
                error,
                brassclaw_llm::LlmError::SessionRenewalFailed { .. }
            ));
        } else {
            assert!(matches!(
                error,
                brassclaw_llm::LlmError::ConfigurationChanged { .. }
            ));
        }
        let client = rig.pool.get().await.unwrap();
        for (sequence, expected) in [
            (
                "codex_preflight_lease_attempts",
                mode == IssuerMode::Success,
            ),
            (
                "codex_preflight_refresh_attempts",
                mode == IssuerMode::NearExpiry,
            ),
        ] {
            let row = client
                .query_one(
                    &format!("SELECT is_called, last_value FROM {sequence}"),
                    &[],
                )
                .await
                .unwrap();
            assert_eq!(row.get::<_, bool>(0), expected);
            if expected {
                assert_eq!(row.get::<_, i64>(1), i64::try_from(index + 1).unwrap());
            }
        }
        assert_eq!(
            client
                .query_one(
                    "SELECT count(*) FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
                    &[&TENANT]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        drop(client);
        assert_eq!(issuer.state.calls.load(Ordering::SeqCst), issuer_calls);
        assert_eq!(
            native
                .read_account(&account.scope, account.id)
                .await
                .unwrap()
                .unwrap()
                .0,
            baseline
        );
    }
}
