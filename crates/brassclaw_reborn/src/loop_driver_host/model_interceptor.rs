//! Capture the authorized, resolved host request immediately before dispatch.
//!
//! This is one model operation, not an agent loop. Callers retain sequencing.
//! Neither Python data nor a reference-only executor snapshot can supply an
//! alternate prompt. Sempai may adjust text behind this host boundary while
//! the existing prefix and provider tool protocol remain intact.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use async_trait::async_trait;
use brassclaw_interceptor::{
    CapturedPrompt, ForensicPacket, InterceptorStore, KohaiUsage, ModelExchangeEvidence,
    PromptSegment, TokenAccountingSnapshot,
};
#[cfg(feature = "root-llm-provider")]
use brassclaw_interceptor::{SempaiProposalSink, SempaiReviewOutcome, SharedInterceptorMode};
use brassclaw_loop_support::{
    HostManagedModelError, HostManagedModelErrorKind, HostManagedModelGateway,
    HostManagedModelMessage, HostManagedModelMessageRole, HostManagedModelRequest,
    HostManagedModelResponse, HostManagedToolResultContent,
};
use brassclaw_turns::run_profile::{LoopCapabilityPort, LoopRunContext, ParentLoopOutput};
#[cfg(feature = "root-llm-provider")]
use brassclaw_turns::{
    LoopMessageRef,
    run_profile::{
        AgentLoopHostErrorKind, LoopModelBudgetAccountant, LoopModelGatewayError,
        LoopModelPolicyGuard, LoopModelResponse, LoopRunContext as ReviewContext, LoopSafeSummary,
        ModelCallOutcome, ModelProfileId, ModelStreamChunk, ModelWorkKind, ModelWorkRequest,
    },
};

pub(super) struct InterceptingModelGateway<G: HostManagedModelGateway + ?Sized> {
    pub(super) gateway: Arc<G>,
    pub(super) run_context: LoopRunContext,
    pub(super) store: Arc<dyn InterceptorStore>,
    pub(super) next_iteration: AtomicU32,
    #[cfg(feature = "root-llm-provider")]
    pub(super) proposal_sink: Arc<dyn SempaiProposalSink>,
    #[cfg(feature = "root-llm-provider")]
    pub(super) sempai_gateway: Option<Arc<dyn HostManagedModelGateway>>,
    #[cfg(feature = "root-llm-provider")]
    pub(super) mode: Option<SharedInterceptorMode>,
    #[cfg(feature = "root-llm-provider")]
    pub(super) accountant: Option<Arc<dyn LoopModelBudgetAccountant>>,
    #[cfg(feature = "root-llm-provider")]
    pub(super) policy_guard: Arc<dyn LoopModelPolicyGuard>,
}

#[async_trait]
impl<G: HostManagedModelGateway + ?Sized> HostManagedModelGateway for InterceptingModelGateway<G> {
    fn supports_tool_exchange(&self) -> bool {
        self.gateway.supports_tool_exchange()
    }

    async fn stream_model(
        &self,
        request: HostManagedModelRequest,
    ) -> Result<HostManagedModelResponse, HostManagedModelError> {
        let (request, packet) = self.prepare(request).await?;
        let result = self.gateway.stream_model(request).await;
        self.finish(packet, &result).await;
        result
    }

    async fn stream_model_with_capabilities(
        &self,
        request: HostManagedModelRequest,
        capabilities: Arc<dyn LoopCapabilityPort>,
    ) -> Result<HostManagedModelResponse, HostManagedModelError> {
        let (request, packet) = self.prepare(request).await?;
        // Forward the exact filtered capability port, not the factory surface.
        let result = self
            .gateway
            .stream_model_with_capabilities(request, capabilities)
            .await;
        self.finish(packet, &result).await;
        result
    }
}

impl<G: HostManagedModelGateway + ?Sized> InterceptingModelGateway<G> {
    async fn prepare(
        &self,
        request: HostManagedModelRequest,
    ) -> Result<(HostManagedModelRequest, ForensicPacket), HostManagedModelError> {
        if request.run_id != self.run_context.run_id || request.turn_id != self.run_context.turn_id
        {
            return Err(HostManagedModelError::new(
                HostManagedModelErrorKind::InvalidRequest,
                "interceptor run mismatch",
            ));
        }
        // PostgreSQL stores iteration as int4. Exhaustion is an explicit error,
        // never wrap/reuse an iteration or truncate its identity.
        let mut iteration = self.next_iteration.load(Ordering::Acquire);
        loop {
            let next = iteration
                .checked_add(1)
                .filter(|next| *next <= i32::MAX as u32)
                .ok_or_else(|| {
                    HostManagedModelError::new(
                        HostManagedModelErrorKind::InvalidRequest,
                        "interceptor iteration exhausted",
                    )
                })?;
            match self.next_iteration.compare_exchange_weak(
                iteration,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => iteration = actual,
            }
        }
        let mut packet = capture(&request, iteration)?;
        packet.model_exchange = Some(ModelExchangeEvidence {
            format: "host-model-exchange/1".into(),
            original_request_bytes: request_snapshot(&request)?,
            effective_request_bytes: None,
            response_bytes: None,
            failure: None,
        });
        #[cfg(feature = "root-llm-provider")]
        let mut request = request;
        self.save(&packet, "assembled prompt").await;
        #[cfg(feature = "root-llm-provider")]
        if let (Some(gateway), Some(mode)) = (self.sempai_gateway.as_ref(), self.mode.as_ref())
            && mode.get() == brassclaw_interceptor::InterceptorMode::Rerouting
        {
            let review = match self.review(&request, &packet, gateway).await {
                Ok(review) => review,
                Err(error) => {
                    self.record_failure(packet, "sempai_preparation", &error)
                        .await;
                    return Err(error);
                }
            };
            request.messages = match recompose(&request.messages, &review, packet.id.as_str()) {
                Ok(messages) => messages,
                Err(error) => {
                    self.record_failure(packet, "sempai_recomposition", &error)
                        .await;
                    return Err(error);
                }
            };
            // Review is not completion of the pending Kohai call.
            packet.sempai_review = Some(review);
            packet.status = brassclaw_interceptor::PacketStatus::SempaiReviewed;
            self.save(&packet, "Sempai review").await;
        }
        if let Some(exchange) = packet.model_exchange.as_mut() {
            exchange.effective_request_bytes = Some(request_snapshot(&request)?);
        }
        self.save(&packet, "effective host request").await;
        Ok((request, packet))
    }

    async fn save(&self, packet: &ForensicPacket, stage: &str) {
        if let Err(error) = self.store.save(packet).await {
            tracing::warn!(packet_id = %packet.id, stage, error = %error,
                "interceptor persistence failed");
        }
    }

    async fn finish(
        &self,
        packet: ForensicPacket,
        result: &Result<HostManagedModelResponse, HostManagedModelError>,
    ) {
        match result {
            Ok(response) => self.close(packet, response).await,
            Err(error) => self.record_failure(packet, "kohai_gateway", error).await,
        }
    }

    async fn record_failure(
        &self,
        mut packet: ForensicPacket,
        stage: &str,
        error: &HostManagedModelError,
    ) {
        if let Some(exchange) = packet.model_exchange.as_mut() {
            exchange.failure = Some(serde_json::json!({
                "stage": stage, "kind": error.kind, "reason_kind": error.reason_kind,
                "observed_at": chrono::Utc::now(),
                "kohai_gateway_entered": stage == "kohai_gateway",
                // A host error is not evidence that the provider did no work.
                "provider_outcome": "unresolved",
            }));
        }
        // Legacy status/completed_at denote response/review, not failure.
        // Keep them unchanged; the failure observation has its own timestamp.
        self.save(&packet, stage).await;
    }

    async fn close(&self, mut packet: ForensicPacket, response: &HostManagedModelResponse) {
        if let Some(exchange) = packet.model_exchange.as_mut() {
            match serde_json::to_string(response) {
                Ok(bytes) => exchange.response_bytes = Some(bytes),
                Err(error) => {
                    tracing::error!(packet_id = %packet.id, error = %error,
                        "interceptor host response encoding failed");
                    return;
                }
            }
        }
        let text = match &response.output {
            ParentLoopOutput::AssistantReply(reply) => reply.content.clone(),
            ParentLoopOutput::CapabilityCalls(_) => match serde_json::to_string(&response.output) {
                Ok(text) => text,
                Err(error) => {
                    tracing::error!(packet_id = %packet.id, error = %error,
                        "interceptor structured response encoding failed");
                    return;
                }
            },
        };
        let usage = response.usage.as_ref().map(|usage| KohaiUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_input_tokens: usage.cache_read_input_tokens,
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
        });
        let closed = if packet.sempai_review.is_some() {
            packet.with_kohai_response_sempai_reviewed(text, usage)
        } else {
            packet.with_kohai_response(text, usage)
        };
        self.save(&closed, "Kohai response").await;
    }
}

/// HostManagedModelMessage's normal serializer intentionally skips Tool replay
/// fields. Capture them explicitly here, without changing its transport contract.
fn request_snapshot(request: &HostManagedModelRequest) -> Result<String, HostManagedModelError> {
    let messages: Vec<_> = request.messages.iter().map(message_snapshot).collect();
    serde_json::to_string(&serde_json::json!({
        "model_profile_id": request.model_profile_id, "messages": messages,
        "surface_version": request.surface_version,
        "resolved_model_route": request.resolved_model_route,
        "run_id": request.run_id, "turn_id": request.turn_id,
    }))
    .map_err(|_| {
        HostManagedModelError::new(
            HostManagedModelErrorKind::InvalidRequest,
            "host request evidence encoding failed",
        )
    })
}

fn message_snapshot(message: &HostManagedModelMessage) -> serde_json::Value {
    let tool_content = match &message.tool_result_content {
        None => serde_json::Value::Null,
        Some(HostManagedToolResultContent::Reference { envelope }) => {
            serde_json::json!({"kind": "reference", "envelope": envelope})
        }
        Some(HostManagedToolResultContent::Resolved { safe_summary }) => {
            serde_json::json!({"kind": "resolved", "safe_summary": safe_summary})
        }
    };
    serde_json::json!({
        "role": message.role, "content": message.content,
        "content_ref": message.content_ref,
        "tool_result_provider_call": message.tool_result_provider_call,
        "tool_result_content": tool_content,
    })
}

#[cfg(all(test, feature = "root-llm-provider"))]
mod tests {
    use super::*;
    use brassclaw_threads::ProviderToolCallReferenceEnvelope;

    fn review(tail: &[(String, String)]) -> SempaiReviewOutcome {
        SempaiReviewOutcome {
            adjusted_volatile_messages: tail.to_vec(),
            bridge_messages: vec![("system".into(), "review bridge".into())],
            composition_summary: "protocol preservation".into(),
            proposed_recipe_updates: Vec::new(),
            proposed_intent_examples: Vec::new(),
            settings_adjustments: Vec::new(),
            proposed_components: Vec::new(),
        }
    }

    #[test]
    fn model_interceptor_keeps_exact_prefix_and_provider_replay_metadata() {
        let base =
            review_message("system", "selected compiled prefix", "packet", "base", 0).unwrap();
        let user = review_message("user", "question", "packet", "input", 0).unwrap();
        let mut tool =
            review_message("assistant", "provider result", "packet", "result", 0).unwrap();
        tool.role = HostManagedModelMessageRole::ToolResult;
        tool.tool_result_provider_call = Some(ProviderToolCallReferenceEnvelope {
            provider_id: "provider".into(),
            provider_model_id: "model".into(),
            provider_turn_id: "provider-turn".into(),
            provider_call_id: "provider-call".into(),
            provider_tool_name: "echo".into(),
            capability_id: brassclaw_host_api::CapabilityId::new("demo.echo").unwrap(),
            arguments: serde_json::json!({"message": "original input"}),
            response_reasoning: Some("response reasoning".into()),
            reasoning: Some("call reasoning".into()),
            signature: Some("provider-signature".into()),
        });
        let original = vec![base.clone(), user.clone(), tool.clone()];
        // Ordinary message serialization deliberately omits replay metadata.
        assert!(
            serde_json::to_value(&tool)
                .unwrap()
                .get("tool_result_provider_call")
                .is_none()
        );
        let snapshot = message_snapshot(&tool);
        assert_eq!(
            snapshot["tool_result_provider_call"]["provider_call_id"],
            "provider-call"
        );
        assert_eq!(
            snapshot["tool_result_provider_call"]["arguments"]["message"],
            "original input"
        );
        assert_eq!(
            snapshot["tool_result_provider_call"]["signature"],
            "provider-signature"
        );
        let review = review(&message_pairs(&original[1..]));
        let adjusted = recompose(&original, &review, "packet").unwrap();
        assert_eq!(adjusted[0], base);
        assert_eq!(adjusted[2], user);
        assert_eq!(adjusted[3], tool);
        assert_eq!(adjusted[1].content, "review bridge");
        let mut broken = review.clone();
        broken.adjusted_volatile_messages.pop();
        assert_eq!(
            recompose(&original, &broken, "packet").unwrap_err().kind,
            HostManagedModelErrorKind::InvalidOutput
        );
        broken = review;
        broken.adjusted_volatile_messages.reverse();
        assert!(recompose(&original, &broken, "packet").is_err());
    }

    #[test]
    fn model_interceptor_rejects_invented_tool_results_and_dropped_system_context() {
        let base = review_message("system", "selected base", "packet", "base", 0).unwrap();
        let user = review_message("user", "question", "packet", "input", 0).unwrap();
        let original = vec![base.clone(), user.clone()];
        let mut adjustment = review(&[("tool_result_reference".into(), "invented result".into())]);
        assert!(recompose(&original, &adjustment, "packet").is_err());
        adjustment = review(&[]);
        assert!(recompose(&original, &adjustment, "packet").is_err());
        let interleaved = vec![
            base,
            user,
            review_message("system", "bound instruction", "packet", "context", 0).unwrap(),
        ];
        adjustment = review(&[("user".into(), "changed question".into())]);
        assert!(recompose(&interleaved, &adjustment, "packet").is_err());
        adjustment = review(&message_pairs(&interleaved[1..]));
        let kept = recompose(&interleaved, &adjustment, "packet").unwrap();
        assert_eq!(kept[2].content, interleaved[1].content);
        assert_eq!(kept[3], interleaved[2]);
        adjustment.adjusted_volatile_messages[0].1 = "changed question".into();
        let changed = recompose(&interleaved, &adjustment, "packet").unwrap();
        assert_eq!(changed[2].content, "changed question");
        assert_eq!(changed[3], interleaved[2]);
    }
}

fn role_name(role: HostManagedModelMessageRole) -> &'static str {
    match role {
        HostManagedModelMessageRole::System => "system",
        HostManagedModelMessageRole::User => "user",
        HostManagedModelMessageRole::Assistant => "assistant",
        HostManagedModelMessageRole::ToolResult => "tool_result_reference",
    }
}

fn message_pairs(messages: &[HostManagedModelMessage]) -> Vec<(String, String)> {
    messages
        .iter()
        .map(|message| (role_name(message.role).into(), message.content.clone()))
        .collect()
}

fn capture(
    request: &HostManagedModelRequest,
    iteration: u32,
) -> Result<ForensicPacket, HostManagedModelError> {
    let message_count = u32::try_from(request.messages.len()).map_err(|_| {
        HostManagedModelError::new(
            HostManagedModelErrorKind::InvalidRequest,
            "prompt message count exceeded",
        )
    })?;
    let mut total = 0_u32;
    let mut segments = Vec::with_capacity(request.messages.len());
    for (index, message) in request.messages.iter().enumerate() {
        let estimate =
            u32::try_from(message.content.chars().count().div_ceil(4)).map_err(|_| {
                HostManagedModelError::new(
                    HostManagedModelErrorKind::InvalidRequest,
                    "prompt accounting exceeded",
                )
            })?;
        total = total.checked_add(estimate).ok_or_else(|| {
            HostManagedModelError::new(
                HostManagedModelErrorKind::InvalidRequest,
                "prompt accounting exceeded",
            )
        })?;
        segments.push(PromptSegment {
            label: format!("resolved_message_{index}"),
            content: message.content.clone(),
            estimated_tokens: estimate,
            inclusion_reason: "authorized host prompt resolution".into(),
            component_uuid: None,
        });
    }
    Ok(ForensicPacket::new(
        request.run_id.to_string(),
        iteration,
        CapturedPrompt {
            messages: message_pairs(&request.messages),
            segments,
            token_accounting: TokenAccountingSnapshot {
                // These limits are not available at this boundary; 0 means unknown
                // here, never an enforced token cap or claimed provider KV proof.
                context_window_limit: 0,
                max_output_tokens: 0,
                total_input_estimated: total,
                message_count,
                kv_cache_optimised: false,
            },
            capability_surface_version: request
                .surface_version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            visible_capability_count: 0,
        },
    ))
}

#[cfg(feature = "root-llm-provider")]
fn recompose(
    original: &[HostManagedModelMessage],
    review: &SempaiReviewOutcome,
    packet_id: &str,
) -> Result<Vec<HostManagedModelMessage>, HostManagedModelError> {
    let base_length = original
        .iter()
        .take_while(|message| message.role == HostManagedModelMessageRole::System)
        .count();
    let tail = &original[base_length..];
    let mut messages = original[..base_length].to_vec();
    // The tuple-only review contract cannot describe provider call IDs or
    // result envelopes. Keep the complete tail exactly when those are present.
    let protocol_tail = tail.iter().any(|message| {
        message.role == HostManagedModelMessageRole::ToolResult
            || message.tool_result_provider_call.is_some()
            || message.tool_result_content.is_some()
    });
    if protocol_tail && review.adjusted_volatile_messages != message_pairs(tail) {
        return Err(HostManagedModelError::new(
            HostManagedModelErrorKind::InvalidOutput,
            "Sempai cannot replace provider replay or system context with text tuples",
        ));
    }
    for (index, (role, content)) in review.bridge_messages.iter().enumerate() {
        messages.push(review_message(role, content, packet_id, "bridge", index)?);
    }
    if protocol_tail {
        messages.extend_from_slice(tail);
    } else if tail
        .iter()
        .any(|message| message.role == HostManagedModelMessageRole::System)
    {
        // Some host materializations insert protected system context among the
        // history messages. Preserve its exact position/content/reference;
        // text-only messages around it may change within the same role layout.
        if review.adjusted_volatile_messages.len() != tail.len() {
            return Err(HostManagedModelError::new(
                HostManagedModelErrorKind::InvalidOutput,
                "Sempai adjustment changed protected system context positions",
            ));
        }
        for (index, (original, (role, content))) in tail
            .iter()
            .zip(&review.adjusted_volatile_messages)
            .enumerate()
        {
            if role != role_name(original.role)
                || (original.role == HostManagedModelMessageRole::System
                    && content != &original.content)
            {
                return Err(HostManagedModelError::new(
                    HostManagedModelErrorKind::InvalidOutput,
                    "Sempai adjustment replaced protected system context",
                ));
            }
            messages.push(if original.role == HostManagedModelMessageRole::System {
                original.clone()
            } else {
                review_message(role, content, packet_id, "volatile", index)?
            });
        }
    } else {
        if review.adjusted_volatile_messages.is_empty() && !tail.is_empty() {
            return Err(HostManagedModelError::new(
                HostManagedModelErrorKind::InvalidOutput,
                "Sempai adjustment removed the complete volatile context",
            ));
        }
        for (index, (role, content)) in review.adjusted_volatile_messages.iter().enumerate() {
            messages.push(review_message(role, content, packet_id, "volatile", index)?);
        }
    }
    Ok(messages)
}

#[cfg(feature = "root-llm-provider")]
fn review_message(
    role: &str,
    content: &str,
    packet_id: &str,
    section: &str,
    index: usize,
) -> Result<HostManagedModelMessage, HostManagedModelError> {
    let role = match role {
        "system" => HostManagedModelMessageRole::System,
        "user" => HostManagedModelMessageRole::User,
        "assistant" => HostManagedModelMessageRole::Assistant,
        _ => {
            return Err(HostManagedModelError::new(
                HostManagedModelErrorKind::InvalidOutput,
                "Sempai review message role is unsupported",
            ));
        }
    };
    let content_ref = LoopMessageRef::new(format!("msg:interceptor.{packet_id}.{section}.{index}"))
        .map_err(|_| {
            HostManagedModelError::new(
                HostManagedModelErrorKind::InvalidRequest,
                "Sempai message correlation failed",
            )
        })?;
    Ok(HostManagedModelMessage {
        role,
        content: content.into(),
        content_ref,
        tool_result_provider_call: None,
        tool_result_content: None,
    })
}

#[cfg(feature = "root-llm-provider")]
impl<G: HostManagedModelGateway + ?Sized> InterceptingModelGateway<G> {
    async fn review(
        &self,
        request: &HostManagedModelRequest,
        packet: &ForensicPacket,
        gateway: &Arc<dyn HostManagedModelGateway>,
    ) -> Result<SempaiReviewOutcome, HostManagedModelError> {
        let base_length = request
            .messages
            .iter()
            .take_while(|message| message.role == HostManagedModelMessageRole::System)
            .count();
        // Use the actual selected/resolved base. Looking up the newest bundle
        // during review would silently replace this task's prompt selection.
        let mut messages = request.messages[..base_length].to_vec();
        messages.push(review_message(
            "system",
            super::sempai_persona(),
            packet.id.as_str(),
            "persona",
            0,
        )?);
        let volatile = serde_json::to_string(&message_pairs(&request.messages[base_length..]))
            .map_err(|_| {
                HostManagedModelError::new(
                    HostManagedModelErrorKind::InvalidRequest,
                    "Sempai volatile context encoding failed",
                )
            })?;
        messages.push(review_message(
            "user",
            &volatile,
            packet.id.as_str(),
            "audit",
            0,
        )?);
        let model_profile_id = ModelProfileId::new("sempai_model").map_err(|_| {
            HostManagedModelError::new(
                HostManagedModelErrorKind::ConfigurationError,
                "Sempai profile invalid",
            )
        })?;
        let response = self
            .accounted_review(
                gateway,
                HostManagedModelRequest {
                    model_profile_id,
                    messages,
                    surface_version: None,
                    resolved_model_route: None,
                    run_id: request.run_id,
                    turn_id: request.turn_id,
                },
            )
            .await?;
        let text = match response.output {
            ParentLoopOutput::AssistantReply(reply) => reply.content,
            ParentLoopOutput::CapabilityCalls(_) => {
                return Err(HostManagedModelError::new(
                    HostManagedModelErrorKind::InvalidOutput,
                    "Sempai review cannot dispatch tools",
                ));
            }
        };
        let outcome: SempaiReviewOutcome = serde_json::from_str(&text).map_err(|_| {
            HostManagedModelError::new(
                HostManagedModelErrorKind::InvalidOutput,
                "Sempai returned an invalid review",
            )
        })?;
        // Verify representability before queuing proposals. A text-only review
        // cannot create tool results or discard an actual tool conversation.
        recompose(&request.messages, &outcome, packet.id.as_str())?;
        let packet_id = packet.id.as_str();
        // Route proposed_recipe_updates and proposed_intent_examples to Q1
        // validation queue (non-fatal: failures are logged but do not abort
        // the rerouting pipeline).
        if !outcome.proposed_recipe_updates.is_empty()
            || !outcome.proposed_intent_examples.is_empty()
            || !outcome.proposed_components.is_empty()
        {
            let user_id = self
                .run_context
                .actor
                .as_ref()
                .map(|a| a.user_id.as_str())
                .unwrap_or_default();
            let project_id = self
                .run_context
                .scope
                .project_id
                .as_ref()
                .map(|p| p.as_str())
                .unwrap_or_default();
            match self
                .proposal_sink
                .submit_proposals(
                    user_id,
                    project_id,
                    &outcome.proposed_recipe_updates,
                    &outcome.proposed_intent_examples,
                    &outcome.proposed_components,
                )
                .await
            {
                Ok(result) => {
                    tracing::debug!(
                        packet_id,
                        recipe_updates = result.recipe_updates_queued,
                        intent_examples = result.intent_examples_queued,
                        components = result.components_queued,
                        "interceptor: sempai proposals queued in Q1"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        packet_id,
                        error = %error,
                        "interceptor: sempai proposal submission failed (non-fatal)"
                    );
                }
            }
        }

        Ok(outcome)
    }

    async fn accounted_review(
        &self,
        gateway: &Arc<dyn HostManagedModelGateway>,
        request: HostManagedModelRequest,
    ) -> Result<LoopModelResponse, HostManagedModelError> {
        let accountant = self.accountant.as_ref().ok_or_else(|| {
            HostManagedModelError::new(
                HostManagedModelErrorKind::ConfigurationError,
                "isolated Sempai accounting unavailable",
            )
        })?;
        let chars = request
            .messages
            .iter()
            .try_fold(0_u64, |total, message| {
                total.checked_add(u64::try_from(message.content.chars().count()).ok()?)
            })
            .ok_or_else(|| {
                HostManagedModelError::new(
                    HostManagedModelErrorKind::InvalidRequest,
                    "Sempai input accounting exceeded",
                )
            })?;
        let work = ModelWorkRequest {
            kind: ModelWorkKind::SempaiReview,
            model_profile_id: request.model_profile_id.clone(),
            resolved_model_route: request.resolved_model_route.clone(),
            estimated_input_tokens: chars.div_ceil(4),
            estimated_output_tokens: None,
        };
        self.policy_guard
            .check_model_work_policy(&self.run_context, &work)
            .await
            .map_err(review_work_error)?;
        accountant
            .pre_model_work(&self.run_context, &work)
            .await
            .map_err(review_work_error)?;
        let mut guard = ReviewReservationGuard {
            accountant: accountant.as_ref(),
            context: &self.run_context,
            armed: true,
        };
        let profile = request.model_profile_id.clone();
        let result = gateway
            .stream_model(request)
            .await
            .map(|response| LoopModelResponse {
                chunks: response
                    .safe_text_deltas
                    .into_iter()
                    .map(|safe_text_delta| ModelStreamChunk { safe_text_delta })
                    .collect(),
                safe_reasoning_deltas: response.safe_reasoning_deltas,
                output: response.output,
                effective_model_profile_id: profile,
                usage: response.usage,
            })
            .map_err(review_provider_error);
        let outcome = match &result {
            Ok(response) => ModelCallOutcome::Success(response),
            Err(error) => ModelCallOutcome::Failure(error),
        };
        let recorded = accountant
            .post_model_work_result(&self.run_context, &work, outcome)
            .await;
        // A failed reconcile retains its actual reservation for recovery. Do
        // not release known completed spend merely because recording failed.
        guard.armed = false;
        recorded.map_err(review_work_error)?;
        result.map_err(review_work_error)
    }
}

#[cfg(feature = "root-llm-provider")]
struct ReviewReservationGuard<'a> {
    accountant: &'a dyn LoopModelBudgetAccountant,
    context: &'a ReviewContext,
    armed: bool,
}
#[cfg(feature = "root-llm-provider")]
impl Drop for ReviewReservationGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.accountant.release_in_flight(self.context);
        }
    }
}

#[cfg(feature = "root-llm-provider")]
fn review_work_error(error: LoopModelGatewayError) -> HostManagedModelError {
    let kind = match error.kind {
        AgentLoopHostErrorKind::PolicyDenied => HostManagedModelErrorKind::PolicyDenied,
        AgentLoopHostErrorKind::BudgetExceeded => HostManagedModelErrorKind::BudgetExceeded,
        AgentLoopHostErrorKind::BudgetApprovalRequired => {
            HostManagedModelErrorKind::BudgetApprovalRequired
        }
        AgentLoopHostErrorKind::BudgetAccountingFailed => {
            HostManagedModelErrorKind::BudgetAccountingFailed
        }
        AgentLoopHostErrorKind::Cancelled => HostManagedModelErrorKind::Cancelled,
        AgentLoopHostErrorKind::CredentialUnavailable => {
            HostManagedModelErrorKind::CredentialUnavailable
        }
        AgentLoopHostErrorKind::InvalidInvocation => HostManagedModelErrorKind::InvalidRequest,
        _ => HostManagedModelErrorKind::Unavailable,
    };
    let mut result = HostManagedModelError::new(kind, "Sempai provider work failed");
    result.reason_kind = error.reason_kind;
    result
}

#[cfg(feature = "root-llm-provider")]
fn review_provider_error(error: HostManagedModelError) -> LoopModelGatewayError {
    let kind = match error.kind {
        HostManagedModelErrorKind::InvalidRequest => AgentLoopHostErrorKind::InvalidInvocation,
        HostManagedModelErrorKind::PolicyDenied => AgentLoopHostErrorKind::PolicyDenied,
        HostManagedModelErrorKind::BudgetExceeded => AgentLoopHostErrorKind::BudgetExceeded,
        HostManagedModelErrorKind::BudgetApprovalRequired => {
            AgentLoopHostErrorKind::BudgetApprovalRequired
        }
        HostManagedModelErrorKind::BudgetAccountingFailed => {
            AgentLoopHostErrorKind::BudgetAccountingFailed
        }
        HostManagedModelErrorKind::Cancelled => AgentLoopHostErrorKind::Cancelled,
        HostManagedModelErrorKind::CredentialUnavailable => {
            AgentLoopHostErrorKind::CredentialUnavailable
        }
        _ => AgentLoopHostErrorKind::Unavailable,
    };
    LoopModelGatewayError {
        kind,
        safe_summary: LoopSafeSummary::model_gateway_failed(),
        reason_kind: error.reason_kind,
        diagnostic_ref: None,
    }
}
