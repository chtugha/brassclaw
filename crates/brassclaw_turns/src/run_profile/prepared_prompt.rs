//! Host-owned model-input data, distinct from log/status summaries. There is no
//! Python transport entry point here, source interpolation, provider call or
//! Tool grant. The consuming selected usage owns data admission and sequencing.
use std::fmt;

use crate::LoopMessageRef;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    AgentLoopHostError, AgentLoopHostErrorKind, InstructionBundle, InstructionBundleBuilder,
    InstructionBundleMaterializedMessage, InstructionBundleRequest, InstructionSafetyContext,
    LoopContextBundle, LoopContextMessage, LoopInlineMessageRole, LoopPromptBundleRequest,
    LoopRunContext, VisibleCapabilitySurface,
};

/// Identity of one host-materialized message set, not an effect or grant.
/// Host adapters may explicitly correlate its UUID with their native operation;
/// the prompt contract never accepts a lower runtime authority identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopPromptMessageSetId(Uuid);

impl LoopPromptMessageSetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for LoopPromptMessageSetId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PreparedLoopPromptMessage {
    pub role: LoopInlineMessageRole,
    pub content: String,
}
impl fmt::Debug for PreparedLoopPromptMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedLoopPromptMessage")
            .field("role", &self.role)
            .field("content_bytes", &self.content.len())
            .finish()
    }
}

/// Rust host interface only: existing prompt metadata and complete message data.
/// No Deserialize implementation exposes raw prepared bodies to root port JSON.
/// The message-set identity is correlation, never authorization or approval.
#[derive(Clone)]
pub struct PreparedLoopPromptRequest {
    pub message_set_id: LoopPromptMessageSetId,
    pub metadata: LoopPromptBundleRequest,
    pub messages: Vec<PreparedLoopPromptMessage>,
}
impl fmt::Debug for PreparedLoopPromptRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedLoopPromptRequest")
            .field("message_set_id", &self.message_set_id)
            .field("message_count", &self.messages.len())
            .finish()
    }
}

fn capacity() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Unavailable,
        "prepared prompt capacity unavailable",
    )
}

pub(super) fn build(
    context: &LoopRunContext,
    request: PreparedLoopPromptRequest,
    visible_surface: Option<VisibleCapabilitySurface>,
    safety_context: Option<InstructionSafetyContext>,
) -> Result<InstructionBundle, AgentLoopHostError> {
    build_with_context(
        context,
        request,
        visible_surface,
        safety_context,
        LoopContextBundle::default(),
    )
}

/// Materialize host-owned prefix data together with the already loaded context.
/// Existing transcript references, identity, instructions and memory are retained.
pub(super) fn build_with_context(
    context: &LoopRunContext,
    request: PreparedLoopPromptRequest,
    visible_surface: Option<VisibleCapabilitySurface>,
    safety_context: Option<InstructionSafetyContext>,
    mut context_bundle: LoopContextBundle,
) -> Result<InstructionBundle, AgentLoopHostError> {
    if request.message_set_id.as_uuid().is_nil() || context.run_id.as_uuid().is_nil() {
        return Err(AgentLoopHostError::new(
            AgentLoopHostErrorKind::InvalidInvocation,
            "prepared prompt identity is invalid",
        ));
    }
    let scope = serde_json::to_vec(&context.scope).map_err(|_| capacity())?;
    let mut materialized = Vec::new();
    let mut messages = Vec::new();
    materialized
        .try_reserve_exact(request.messages.len())
        .map_err(|_| capacity())?;
    messages
        .try_reserve_exact(request.messages.len())
        .map_err(|_| capacity())?;
    for (ordinal, message) in request.messages.into_iter().enumerate() {
        let ordinal = u64::try_from(ordinal).map_err(|_| capacity())?;
        let role = message.role.as_str();
        let mut hash = Sha256::new();
        // Unambiguous length-delimited role/body under an explicit profile.
        hash.update(b"prepared-model-message/1\0");
        hash.update(context.run_id.as_uuid().as_bytes());
        hash.update(request.message_set_id.as_uuid().as_bytes());
        hash.update(
            u64::try_from(scope.len())
                .map_err(|_| capacity())?
                .to_be_bytes(),
        );
        hash.update(&scope);
        hash.update(ordinal.to_be_bytes());
        hash.update(
            u64::try_from(role.len())
                .map_err(|_| capacity())?
                .to_be_bytes(),
        );
        hash.update(role.as_bytes());
        hash.update(
            u64::try_from(message.content.len())
                .map_err(|_| capacity())?
                .to_be_bytes(),
        );
        hash.update(message.content.as_bytes());
        let reference = LoopMessageRef::new(format!(
            "msg:prepared.{}.{ordinal}.{:x}",
            request.message_set_id.as_uuid(),
            hash.finalize()
        ))
        .map_err(|_| {
            AgentLoopHostError::new(
                AgentLoopHostErrorKind::Internal,
                "prepared prompt reference unavailable",
            )
        })?;
        messages.push(LoopContextMessage {
            message_ref: Some(reference.clone()),
            role: role.into(),
            safe_summary: "Prepared model message".into(),
            compaction: None,
        });
        materialized.push(InstructionBundleMaterializedMessage {
            role: role.into(),
            content_ref: reference,
            model_content: message.content,
        });
    }
    context_bundle
        .messages
        .try_reserve(messages.len())
        .map_err(|_| capacity())?;
    context_bundle.messages.splice(0..0, messages);
    let mut bundle =
        InstructionBundleBuilder::new(context.clone()).build(InstructionBundleRequest {
            context_bundle,
            visible_surface,
            safety_context,
            inline_messages: request.metadata.inline_messages,
        })?;
    bundle
        .materialized_messages
        .try_reserve(materialized.len())
        .map_err(|_| capacity())?;
    bundle.materialized_messages.extend(materialized);
    bundle.requires_materialization_store = true;
    Ok(bundle)
}
