#![cfg(all(feature = "root-llm-provider", feature = "postgres"))]

#[path = "common/native_pg.rs"]
mod native_pg;

use std::sync::Arc;

use brassclaw_product_adapters::{
    AdapterInstallationId, AuthRequirement, ExternalActorRef, ExternalConversationRef,
    ExternalEventId, InboundCommandPayload, ProductAdapterError, ProductAdapterId,
    ProductInboundAck, ProductInboundEnvelope, ProductInboundPayload, ProductTriggerReason,
    ProductWorkflow, ProtocolAuthEvidence, TrustedInboundContext,
};
use brassclaw_product_workflow::{
    DefaultProductWorkflow, FakeConversationBindingService, FakeIdempotencyLedger,
    FakeInboundTurnService, ProductCommandAdmission, ProductCommandAdmissionService,
    ProductCommandContext, ProductWorkflowError,
};
use brassclaw_reborn_composition::{RebornProviderAdmin, RebornProviderAdminProductCommandService};
use chrono::Utc;

fn sample_command_envelope(
    event_suffix: &str,
    command: &str,
    arguments: &str,
) -> ProductInboundEnvelope {
    let adapter_id = ProductAdapterId::new("test_adapter").expect("valid adapter");
    let installation_id = AdapterInstallationId::new("install_alpha").expect("valid installation");
    let evidence = ProtocolAuthEvidence::test_verified(
        AuthRequirement::SharedSecretHeader {
            header_name: "X-Secret".into(),
        },
        installation_id.as_str(),
    );
    let context = TrustedInboundContext::from_verified_evidence(
        adapter_id,
        installation_id,
        Utc::now(),
        &evidence,
    )
    .expect("verified");
    let parsed = brassclaw_product_adapters::ParsedProductInbound::new(
        ExternalEventId::new(format!("evt:{event_suffix}")).expect("valid event"),
        ExternalActorRef::new("test", "user1", Option::<String>::None).expect("valid actor"),
        ExternalConversationRef::new(None, "conv1", None, None).expect("valid conversation"),
        ProductInboundPayload::Command(
            InboundCommandPayload::new(command, arguments, ProductTriggerReason::BotCommand)
                .expect("valid command"),
        ),
    )
    .expect("parsed");

    ProductInboundEnvelope::from_trusted_parse(context, parsed).expect("envelope")
}

struct AllowingCommandAdmissionService;

#[async_trait::async_trait]
impl ProductCommandAdmissionService for AllowingCommandAdmissionService {
    async fn admit(
        &self,
        _context: &ProductCommandContext,
        _command: &brassclaw_product_workflow::ProductCommand,
    ) -> Result<ProductCommandAdmission, ProductWorkflowError> {
        Ok(ProductCommandAdmission::Allowed)
    }
}

fn workflow_for_pool(
    pool: &brassclaw_pg::PgPool,
) -> (DefaultProductWorkflow, Arc<FakeInboundTurnService>) {
    let admin = RebornProviderAdmin::new(pool.clone(), "default");
    let command_service = Arc::new(RebornProviderAdminProductCommandService::new(admin));
    let inbound = Arc::new(FakeInboundTurnService::new());
    let ledger = Arc::new(FakeIdempotencyLedger::new());
    let binding = Arc::new(FakeConversationBindingService::new());
    let workflow = DefaultProductWorkflow::new(inbound.clone(), ledger, binding)
        .with_product_command_admission_service(Arc::new(AllowingCommandAdmissionService))
        .with_product_command_service(command_service);
    (workflow, inbound)
}

#[tokio::test]
async fn model_provider_command_set_provider_returns_transient_error() {
    // Phase 8: set_provider via product command is no longer file-based.
    // It returns a Transient error directing users to the WebUI or CLI.
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let rig = native_pg::NativePostgres::start().await;
    let (workflow, inbound) = workflow_for_pool(&rig.pool);
    let envelope = sample_command_envelope(
        "command-model-provider",
        "model",
        "set-provider openai --model gpt-5-mini",
    );

    let err = workflow
        .accept_inbound(envelope)
        .await
        .expect_err("set-provider should return transient error");

    assert!(matches!(err, ProductAdapterError::WorkflowTransient { .. }));
    assert_eq!(inbound.accepted_count(), 0);
    // No config.toml should have been written.
    assert!(
        !reborn_home.join("config.toml").exists(),
        "set-provider must not write config.toml in DB-backed mode"
    );
}

#[tokio::test]
async fn model_set_command_returns_transient_error() {
    // Phase 8: set_model via product command is no longer file-based.
    // It returns a Transient error directing users to the WebUI or CLI.
    let rig = native_pg::NativePostgres::start().await;
    let (workflow, inbound) = workflow_for_pool(&rig.pool);
    let envelope = sample_command_envelope("command-model-set", "model", "gpt-5.3-codex");

    let err = workflow
        .accept_inbound(envelope)
        .await
        .expect_err("set should return transient error");

    assert!(matches!(err, ProductAdapterError::WorkflowTransient { .. }));
    assert_eq!(inbound.accepted_count(), 0);
}

#[tokio::test]
async fn model_provider_command_rejects_unknown_provider_as_invalid_binding() {
    // Phase 8: set_provider is no longer dispatched to the provider catalog —
    // it returns Transient before the unknown-provider check.  This test now
    // verifies that set-provider via product command produces a Transient error
    // regardless of whether the provider is known.
    let rig = native_pg::NativePostgres::start().await;
    let (workflow, inbound) = workflow_for_pool(&rig.pool);
    let envelope = sample_command_envelope(
        "command-model-provider-unknown",
        "model",
        "set-provider missing-provider",
    );

    let err = workflow
        .accept_inbound(envelope)
        .await
        .expect_err("set-provider should return transient error");

    assert!(matches!(err, ProductAdapterError::WorkflowTransient { .. }));
    assert_eq!(inbound.accepted_count(), 0);
}

#[tokio::test]
async fn non_model_command_is_rejected_by_provider_admin_service() {
    let rig = native_pg::NativePostgres::start().await;
    let (workflow, inbound) = workflow_for_pool(&rig.pool);
    let envelope = sample_command_envelope("command-status-provider-admin", "status", "");

    let ack = workflow
        .accept_inbound(envelope)
        .await
        .expect("non-model command should produce rejection ack");

    let ProductInboundAck::Rejected(rejection) = ack else {
        panic!("expected provider-admin rejection");
    };
    assert_eq!(
        rejection.kind,
        brassclaw_product_adapters::ProductRejectionKind::PolicyDenied
    );
    assert_eq!(inbound.accepted_count(), 0);
}

#[tokio::test]
async fn provider_command_status_uses_real_database_and_redacts_connection_fields() {
    use brassclaw_product_workflow::{
        ActionFingerprintKey, ProductActionId, ProductCommand, ProductCommandService,
        ProductModelCommand, SourceBindingKey,
    };
    use brassclaw_reborn_composition::db_config::{ConfigWriteContext, save_config_key};
    let rig = native_pg::NativePostgres::start().await;
    let definition = serde_json::from_value(serde_json::json!({
        "id": "product-db", "protocol": "open_ai_completions", "model_env": "PRODUCT_MODEL",
        "default_model": "db-default", "description": "Database product fixture"
    }))
    .unwrap();
    brassclaw_reborn_composition::PgProviderRepo::new((*rig.pool).clone(), "default")
        .upsert(definition)
        .await
        .unwrap();
    for (key, value) in [
        ("llm.default.provider_id", "product-db"),
        ("llm.default.model", "product-selected"),
        ("llm.default.api_key_env", "PRODUCT_SECRET_REF"),
        ("llm.default.base_url", "https://private.example/v1"),
    ] {
        save_config_key(
            &rig.pool,
            "default",
            key,
            value,
            ConfigWriteContext::Operator,
        )
        .await
        .unwrap();
    }
    let service = RebornProviderAdminProductCommandService::new(RebornProviderAdmin::new(
        (*rig.pool).clone(),
        "default",
    ));
    let envelope = sample_command_envelope("native-status", "model", "status");
    let fingerprint = ActionFingerprintKey::new(
        envelope.adapter_id().clone(),
        envelope.installation_id().clone(),
        envelope.external_actor_ref().clone(),
        SourceBindingKey::new(envelope.source_binding_key()).unwrap(),
        envelope.external_event_id().clone(),
    );
    let context =
        ProductCommandContext::from_envelope(&envelope, ProductActionId::new(), fingerprint)
            .unwrap();
    let action = ProductCommand::Model {
        action: ProductModelCommand::Status,
    };
    let ack = service
        .execute(context.clone(), action.clone())
        .await
        .unwrap();
    let value = serde_json::to_value(ack).unwrap();
    let text = value.to_string();
    assert!(text.contains("product-selected"));
    assert!(text.contains("product-db"));
    assert!(!text.contains("PRODUCT_SECRET_REF"));
    assert!(!text.contains("private.example"));
    let client = rig.pool.get().await.unwrap();
    client
        .batch_execute("ALTER TABLE brassclaw_llm_providers RENAME TO unavailable_provider_rows")
        .await
        .unwrap();
    assert!(matches!(
        service.execute(context, action).await,
        Err(ProductWorkflowError::Transient { .. })
    ));
    client
        .batch_execute("ALTER TABLE unavailable_provider_rows RENAME TO brassclaw_llm_providers")
        .await
        .unwrap();
}
