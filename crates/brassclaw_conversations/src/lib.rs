//! Conversation binding and session-thread contracts for BrassClaw Reborn.
//!
//! This crate is the adapter-safe boundary between product/channel adapters and
//! `brassclaw_turns::TurnCoordinator`. It resolves external actor/conversation
//! identifiers into canonical tenant/thread/message/binding references without
//! asking the turn coordinator to parse raw channel payloads or store message
//! content.

mod error;
mod filesystem_store;
mod ids;
mod inbound;
mod memory;
mod pg_store;
mod state_store;
mod traits;
mod trusted_trigger;
mod types;

pub use error::InboundTurnError;
pub use filesystem_store::{
    FilesystemConversationStateStore, RebornFilesystemConversationServices,
};
pub use pg_store::PgConversationStateStore;
pub use ids::{
    AdapterInstallationId, AdapterKind, ExternalActorRef, ExternalConversationIdentity,
    ExternalConversationRef, ExternalEventId, InboundMessageContentRef,
};
pub use inbound::{InboundTurnService, trusted_trigger_fire_submitter};
pub use memory::InMemoryConversationServices;
pub use traits::{
    ConversationActorPairingService, ConversationBindingService, ConversationBindingServiceExt,
    SessionThreadService,
};
pub use types::{
    AcceptInboundMessageRequest, AcceptedInboundMessage, AcceptedInboundMessageLookup,
    AcceptedInboundMessageReplay, ConversationBindingResolution, ConversationRouteKind,
    InboundTurnRequest, InboundTurnResponse, LinkConversationRequest, LinkedConversationBinding,
    MessageIdempotencyStatus, ReplyTargetBinding, ResolveConversationRequest, ThreadAccessDecision,
    ThreadMessageRecord, ValidateReplyTargetRequest,
};
