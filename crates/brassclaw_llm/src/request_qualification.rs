//! Native SDK serialization observations for draft Prefix qualification. Chat
//! calls reject at the immutable-body barrier; Codex observes the shared codec
//! without entering a provider and drops its unauthenticated request locally.
//! Neither path supplies token measurements, approvals or dispatch authority.
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
    let samples = qualification_samples();
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

fn qualification_samples() -> [(&'static str, Vec<ChatMessage>, bool); 5] {
    let dynamic = "Qualification instruction: retain the supplied reference and answer the current user request.";
    [
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
    ]
}

/// Unauthenticated observation of the actual Codex request builder. No Debug or
/// Serialize: the body may contain private prompt/history. This owns NO Request,
/// client, credential, measurement, CheckedFinalRequest or dispatch authority.
pub struct CodexRequestObservation {
    pub transport: &'static str,
    pub transport_checksum: String,
    pub endpoint: reqwest::Url,
    pub body: String,
    pub technical_headers: reqwest::header::HeaderMap,
    /// Original caller setting only. The actual Codex body currently carries no
    /// output limit; this must never be interpreted as an effective reservation.
    pub requested_output_tokens: Option<u32>,
}

fn observe_codex(
    config: &crate::config::OpenAiCodexConfig,
    timeout_secs: u64,
    body: serde_json::Value,
    requested_output_tokens: Option<u32>,
    max_bytes: usize,
) -> Result<CodexRequestObservation, crate::LlmError> {
    let reject = |reason| crate::LlmError::FinalRequestRejected { reason };
    if max_bytes == 0 {
        return Err(reject(FinalRequestError::Body));
    }
    brassclaw_common::json::count_json_bytes(&body, Some(max_bytes as u64))
        .map_err(|_| reject(FinalRequestError::Body))?;
    let endpoint = crate::codex_request::endpoint(&config.api_base_url).map_err(reject)?;
    let client = crate::registry_http_client("openai_codex", timeout_secs)?;
    let request = crate::codex_request::build_request(
        &client,
        &endpoint,
        &body,
        crate::codex_request::technical_headers(),
    )?;
    crate::codex_request::verify_technical_headers(request.headers()).map_err(reject)?;
    let body = request
        .body()
        .and_then(reqwest::Body::as_bytes)
        .filter(|body| body.len() <= max_bytes)
        .ok_or_else(|| reject(FinalRequestError::Body))?;
    let body = std::str::from_utf8(body)
        .map_err(|_| reject(FinalRequestError::Body))?
        .to_owned();
    // Consume the observation only; the locally owned request/client are dropped
    // here, with no call to a provider/session, final guard, execute or send.
    Ok(CodexRequestObservation {
        transport: crate::codex_request::TRANSPORT,
        transport_checksum: crate::codex_request::contract_checksum(),
        endpoint,
        body,
        technical_headers: request.headers().clone(),
        requested_output_tokens,
    })
}

/// Production codec/options and actual HTTP serialization, without entering a
/// provider or fabricating a JWT/host session. This is NOT a token measurement.
pub fn codex_completion(
    config: &crate::config::OpenAiCodexConfig,
    timeout_secs: u64,
    request: &CompletionRequest,
    max_bytes: usize,
) -> Result<CodexRequestObservation, crate::LlmError> {
    crate::codex_request::validate_options(
        request.model.as_deref(),
        &config.model,
        request.max_tokens,
        request.temperature,
        request.stop_sequences.as_deref(),
        None,
    )?;
    let body = crate::codex_request::body(&config.model, &request.messages, None)?;
    observe_codex(config, timeout_secs, body, request.max_tokens, max_bytes)
}

pub fn codex_tools(
    config: &crate::config::OpenAiCodexConfig,
    timeout_secs: u64,
    request: &ToolCompletionRequest,
    max_bytes: usize,
) -> Result<CodexRequestObservation, crate::LlmError> {
    crate::codex_request::validate_options(
        request.model.as_deref(),
        &config.model,
        request.max_tokens,
        request.temperature,
        request.stop_sequences.as_deref(),
        request.tool_choice.as_deref(),
    )?;
    let body = crate::codex_request::body(&config.model, &request.messages, Some(&request.tools))?;
    observe_codex(config, timeout_secs, body, request.max_tokens, max_bytes)
}

pub const CODEX_CORPUS: &str = "brassclaw-native-codex-request-corpus/1";
pub struct CodexQualificationBody {
    pub id: &'static str,
    pub observation: CodexRequestObservation,
}
/// Same complete case families as Chat, with a distinct observation type and
/// contract. Cannot be consumed as old Chat corpus/token/approval evidence.
pub fn codex_prefix_bodies(
    config: &crate::config::OpenAiCodexConfig,
    timeout_secs: u64,
    reference: &str,
    requested_output_tokens: Option<u32>,
    tools: bool,
    max_bytes: usize,
) -> Result<Vec<CodexQualificationBody>, crate::LlmError> {
    if reference.is_empty() || max_bytes == 0 {
        return Err(crate::LlmError::FinalRequestRejected {
            reason: FinalRequestError::Body,
        });
    }
    let mut observations = Vec::new();
    let mut remaining = max_bytes;
    for (id, mut messages, with_tools) in qualification_samples() {
        if with_tools && !tools {
            continue;
        }
        messages.insert(0, ChatMessage::system(reference));
        let mut request = CompletionRequest::new(messages);
        request.model = Some(config.model.clone());
        request.max_tokens = requested_output_tokens;
        request.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
        let observation = if with_tools {
            codex_tools(
                config,
                timeout_secs,
                &ToolCompletionRequest::from_completion_request(
                    request,
                    vec![ToolDefinition {
                        name: "prefix_qualification_schema".into(),
                        description: "Schema serialization only; never invoke this Tool.".into(),
                        parameters: serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
                    }],
                ),
                remaining,
            )?
        } else {
            codex_completion(config, timeout_secs, &request, remaining)?
        };
        remaining = remaining.checked_sub(observation.body.len()).ok_or(
            crate::LlmError::FinalRequestRejected {
                reason: FinalRequestError::Body,
            },
        )?;
        observations.push(CodexQualificationBody { id, observation });
    }
    Ok(observations)
}
