//! Native SDK serialization for draft Prefix qualification. Every operation is
//! rejected at the immutable-body barrier, before HTTP submission. These are
//! protocol observations, never token measurements, approvals or model results.
use crate::final_request_guard::{
    FinalRequestError, FinalRequestGuard, FinalRequestInput, FinalRequestMeasurement,
    FinalRequestMeasurer,
};
use crate::{
    ChatMessage, CompletionRequest, ProviderDispatchGuard, ProviderDispatchPolicy,
    ProviderDispatchState, ProviderRequestFacts, RegistryProviderConfig, ToolCall,
    ToolCompletionRequest, ToolDefinition,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

pub const CORPUS: &str = "brassclaw-native-sdk-prefix-corpus/1";
/// Contains original private body bytes. Deliberately no Debug/Serialize.
pub struct QualificationBody {
    pub id: &'static str,
    pub body: String,
}
#[derive(Debug)]
struct NoSubmission;
#[async_trait]
impl ProviderDispatchGuard for NoSubmission {
    async fn check(
        &self,
        _: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, crate::LlmError> {
        Err(crate::LlmError::FinalRequestRejected {
            reason: FinalRequestError::Binding,
        })
    }
}
struct Capture {
    body: Mutex<Option<String>>,
    max_bytes: usize,
}
#[async_trait]
impl FinalRequestMeasurer for Capture {
    async fn measure(
        &self,
        input: FinalRequestInput<'_>,
    ) -> Result<FinalRequestMeasurement, FinalRequestError> {
        if input.body.len() > self.max_bytes {
            return Err(FinalRequestError::Body);
        }
        let body = std::str::from_utf8(input.body).map_err(|_| FinalRequestError::Body)?;
        let mut held = self.body.lock().map_err(|_| FinalRequestError::Body)?;
        if held.is_some() {
            return Err(FinalRequestError::Body);
        }
        *held = Some(body.into());
        // No successful measurement/CheckedFinalRequest can leave this scope.
        Err(FinalRequestError::Binding)
    }
}
/// Exercise the actual selected native SDK builder, including system-preamble
/// merging, history and (when declared supported) Tool-schema serialization.
/// No credential lookup, provider publication, Tool invocation or HTTP send.
pub async fn prefix_bodies(
    config: &RegistryProviderConfig,
    timeout_secs: u64,
    reference: &str,
    output_tokens: u32,
    tools: bool,
    max_bytes: usize,
) -> Result<Vec<QualificationBody>, FinalRequestError> {
    if reference.is_empty() || output_tokens == 0 || max_bytes == 0 {
        return Err(FinalRequestError::Body);
    }
    let endpoint = crate::final_chat_endpoint(config)?;
    let provider = crate::create_registry_provider(config, timeout_secs)
        .map_err(|_| FinalRequestError::UnsupportedTransport)?;
    let dynamic = "Qualification instruction: retain the supplied reference and answer the current user request.";
    let samples = [
        (
            "single-system",
            vec![ChatMessage::user("Current qualification request")],
            false,
        ),
        (
            "system-join",
            vec![
                ChatMessage::system(dynamic),
                ChatMessage::user("Current qualification request"),
            ],
            false,
        ),
        (
            "history",
            vec![
                ChatMessage::system(dynamic),
                ChatMessage::user("Previous user request"),
                ChatMessage::assistant("Previous assistant answer"),
                ChatMessage::user("Current qualification request"),
            ],
            false,
        ),
        (
            "tools-and-history",
            vec![
                ChatMessage::system(dynamic),
                ChatMessage::user("Previous user request"),
                ChatMessage::assistant("Previous assistant answer"),
                ChatMessage::user("Current qualification request"),
            ],
            true,
        ),
        (
            "tool-continuation",
            vec![
                ChatMessage::system(dynamic),
                ChatMessage::user("Previous tool request"),
                ChatMessage::assistant_with_tool_calls(
                    None,
                    vec![ToolCall {
                        id: "prefixQualificationCall".into(),
                        name: "prefix_qualification_schema".into(),
                        arguments: serde_json::json!({}),
                        reasoning: None,
                        signature: None,
                    }],
                ),
                ChatMessage::tool_result(
                    "prefixQualificationCall",
                    "prefix_qualification_schema",
                    "Captured result data only; no Tool was invoked",
                ),
                ChatMessage::user("Continue from the captured result"),
            ],
            true,
        ),
    ];
    let mut observed = Vec::new();
    let mut retained_bytes = 0usize;
    for (id, mut messages, with_tools) in samples {
        if with_tools && !tools {
            continue;
        }
        messages.insert(0, ChatMessage::system(reference));
        let capture = Arc::new(Capture {
            body: Mutex::new(None),
            max_bytes,
        });
        let guard = Arc::new(FinalRequestGuard::new(
            config.provider_id.clone(),
            config.model.clone(),
            endpoint.clone(),
            Arc::new(NoSubmission),
            capture.clone(),
        )?);
        let mut request = CompletionRequest::new(messages);
        request.model = Some(config.model.clone());
        request.max_tokens = Some(output_tokens);
        request.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
        let rejected = if with_tools {
            guard.complete_with_tools(provider.as_ref(),ToolCompletionRequest::from_completion_request(request,vec![ToolDefinition {
                name:"prefix_qualification_schema".into(),description:"Schema serialization only; never invoke this Tool.".into(),
                parameters:serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
            }])).await.is_err()
        } else {
            guard.complete(provider.as_ref(), request).await.is_err()
        };
        if !rejected {
            return Err(FinalRequestError::Measurement);
        }
        let body = capture
            .body
            .lock()
            .map_err(|_| FinalRequestError::Body)?
            .take()
            .ok_or(FinalRequestError::Body)?;
        retained_bytes = retained_bytes
            .checked_add(body.len())
            .filter(|size| *size <= max_bytes)
            .ok_or(FinalRequestError::Body)?;
        observed.push(QualificationBody { id, body });
    }
    Ok(observed)
}
