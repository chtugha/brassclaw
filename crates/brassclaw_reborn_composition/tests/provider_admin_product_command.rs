#![cfg(feature = "root-llm-provider")]

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
use brassclaw_reborn_config::{RebornBootConfig, RebornHome};
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

fn workflow_for_reborn_home(
    reborn_home: &std::path::Path,
) -> (DefaultProductWorkflow, Arc<FakeInboundTurnService>) {
    let home = RebornHome::resolve_from_env_parts(
        Some(reborn_home.as_os_str().to_os_string()),
        None,
        None,
    )
    .expect("valid reborn home");
    let admin = RebornProviderAdmin::new(RebornBootConfig::new(home));
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
    let (workflow, inbound) = workflow_for_reborn_home(&reborn_home);
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
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let (workflow, inbound) = workflow_for_reborn_home(&reborn_home);
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
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let (workflow, inbound) = workflow_for_reborn_home(&reborn_home);
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
    let temp = tempfile::tempdir().expect("tempdir");
    let reborn_home = temp.path().join("reborn-home");
    let (workflow, inbound) = workflow_for_reborn_home(&reborn_home);
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
