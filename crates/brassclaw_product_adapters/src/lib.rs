//! Product-adapter contracts for BrassClaw Reborn.

#![forbid(unsafe_code)]

mod adapter;
pub mod auth;
mod auth_verifier;
pub mod capabilities;
mod egress;
mod egress_policy;
mod error;
pub mod external;
#[cfg(any(test, feature = "test-support"))]
pub mod fakes;
pub mod identity;
pub mod inbound;
mod log_record;
mod outbound;
mod projection;
pub mod redaction;
#[cfg(feature = "host-auth-mint")]
mod runner;
#[cfg(feature = "host-auth-mint")]
mod runner_immediate_ack;
mod workflow;

pub use adapter::{ProductAdapter, ProductAdapterHealth};
pub use auth::{AuthRequirement, ProtocolAuthEvidence, ProtocolAuthFailure, VerifiedAuthClaim};
#[cfg(feature = "host-auth-mint")]
pub use auth::{
    mark_bearer_token_verified, mark_request_signature_verified, mark_session_verified,
    mark_shared_secret_header_verified,
};
pub use capabilities::{ProductAdapterCapabilities, ProductCapabilityFlag};
pub use egress::{
    DeclaredEgressHost, DeclaredEgressTarget, DeliveryAttemptId, DeliveryStatus,
    EgressCredentialHandle, EgressHeader, EgressMethod, EgressPath, EgressRequest, EgressResponse,
    OutboundDeliverySink, ProtocolHttpEgress, ProtocolHttpEgressError,
};
pub use error::{ProductAdapterError, ProductWorkflowRejectionKind};
pub use external::{
    ExternalActorRef, ExternalConversationRef, ExternalEventId, ProductAttachmentDescriptor,
    ProductAttachmentKind,
};
#[cfg(any(test, feature = "test-support"))]
pub use fakes::{
    FakeOutboundDeliverySink, FakeProductWorkflow, FakeProjectionStream, FakeProtocolHttpEgress,
    RecordedEgressCall,
};
pub use identity::{AdapterInstallationId, ProductAdapterId, ProductSurfaceKind};
pub use inbound::{
    ApprovalDecision, ApprovalResolutionPayload, AuthResolutionPayload, AuthResolutionResult,
    InboundCommandPayload, InboundRetryDisposition, LinkedThreadActionPayload,
    ParsedProductInbound, ProductCommandResultPayload, ProductControlActionPayload,
    ProductInboundAck, ProductInboundEnvelope, ProductInboundPayload, ProductRejection,
    ProductRejectionDisposition, ProductRejectionKind, ProductSlashCommandParseError,
    ProductTriggerReason, ProjectionReadPayload, ProjectionSubscriptionPayload,
    ScopedApprovalResolutionPayload, TrustedInboundContext, UserMessagePayload,
    parse_product_slash_command,
};
pub use outbound::{
    AuthPromptChallengeKind, AuthPromptView, CAPABILITY_DISPLAY_KIND_MAX_BYTES,
    CAPABILITY_DISPLAY_PREVIEW_MAX_BYTES, CAPABILITY_DISPLAY_RESULT_REF_MAX_BYTES,
    CAPABILITY_DISPLAY_SUMMARY_MAX_BYTES, CapabilityActivityStatusView, CapabilityActivityView,
    CapabilityActivityViewInput, CapabilityDisplayPreviewView, CapabilityDisplayPreviewViewInput,
    FinalReplyView, GatePromptView, PROJECTION_SKILL_ACTIVATION_MAX_ITEMS,
    PROJECTION_SKILL_FEEDBACK_MAX_BYTES, PROJECTION_SKILL_NAME_MAX_BYTES, ProductOutboundEnvelope,
    ProductOutboundPayload, ProductOutboundTarget, ProductProjectionItem, ProductProjectionState,
    ProductRenderOutcome, ProductSynchronousResponse, ProductWorkSummaryPhase, ProgressKind,
    ProgressUpdateView, ProjectionCursor,
};
pub use projection::{
    ProductProjectionReadInput, ProductProjectionSubject, ProductProjectionSubscribeInput,
    ProjectionReadRequest, ProjectionStream, ProjectionSubscriptionRequest,
};
pub use redaction::{REDACTED_PLACEHOLDER, RedactedDebug, RedactedString};
pub use workflow::ProductWorkflow;

pub use auth_verifier::{
    Clock, HmacWebhookAuth, SharedSecretHeaderAuth, VerificationOutcome, WebhookAuthVerifier,
};
pub use egress_policy::{EgressPolicy, EgressPolicyError, EgressPolicyTarget};
pub use log_record::{ComponentLogLevel, ComponentLogRecord};
#[cfg(feature = "host-auth-mint")]
pub use runner::{
    DEFAULT_MAX_IN_FLIGHT_WEBHOOKS, DEFAULT_WEBHOOK_WORKFLOW_TIMEOUT, NativeProductAdapterRunner,
    NativeProductAdapterRunnerConfig, RunnerError, WebhookAuth, WebhookProcessOutcome,
    evidence_from_bearer_subject, evidence_from_session_subject,
};
#[cfg(feature = "host-auth-mint")]
pub use runner_immediate_ack::ImmediateAckWorkflowObserver;
