//! Credential-free codec/transport and RAW-provider rejection regressions.
//! No network model/issuer operation, token, fake JWT/session source, measured
//! count or accepted Prefix. The raw test does not cover TokenRefreshing/native
//! Auth; that boundary has its separate actual Composition factory regression.
use super::*;
use crate::config::OpenAiCodexConfig;
use crate::provider::{CompletionRequest, ImageUrl, LlmProvider, ToolCall, ToolCompletionRequest};
use crate::request_qualification::{codex_completion, codex_prefix_bodies, codex_tools};
use serde_json::json;

fn config(base: &str, model: &str) -> OpenAiCodexConfig {
    OpenAiCodexConfig {
        model: model.into(),
        api_base_url: base.into(),
        auth_endpoint: String::new(),
        client_id: String::new(),
        session_path: std::path::PathBuf::new(),
        token_refresh_margin_secs: 0,
    }
}

#[test]
fn codex_no_send_preserves_actual_responses_history_tools_and_reasoning() {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!(
        "http://{}/backend-api/codex",
        listener.local_addr().unwrap()
    );
    let config = config(&base, "gpt-5.5");
    let raw_arguments = " {\"query\":\"a\\nü\"} \n";
    let messages = vec![
        ChatMessage::system(" Prefix ü\r\n"),
        ChatMessage::system("Dynamic\n"),
        ChatMessage::user_with_parts(
            "Original",
            vec![ContentPart::Text {
                text: "追加\n".into(),
            }],
        ),
        ChatMessage::assistant_with_tool_calls(
            Some("Let me check\n".into()),
            vec![ToolCall {
                id: "call_original".into(),
                name: "mcp.read".into(),
                arguments: json!(raw_arguments),
                reasoning: None,
                signature: None,
            }],
        ),
        ChatMessage::tool_result("call_original", "mcp.read", " Complete result ü\r\n"),
        ChatMessage::user("Continue"),
    ];
    let mut request = ToolCompletionRequest::new(
        messages,
        vec![ToolDefinition {
            name: "mcp.read".into(),
            description: "Read".into(),
            parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}),
        }],
    );
    request.max_tokens = Some(37);
    let observed = codex_tools(&config, 5, &request, 65536).unwrap();
    let body: serde_json::Value = serde_json::from_str(&observed.body).unwrap();
    assert_eq!(observed.endpoint.as_str(), format!("{base}/responses"));
    assert_eq!(observed.transport, TRANSPORT);
    assert_eq!(observed.transport_checksum, contract_checksum());
    assert_eq!(observed.requested_output_tokens, Some(37));
    assert_eq!(
        body,
        json!({
            "model":"gpt-5.5", "store":false, "stream":true,
            "text":{"verbosity":"medium"},
            "reasoning":{"effort":"medium","summary":"auto"}, "include":["reasoning.encrypted_content"],
            "instructions":" Prefix ü\r\n\n\nDynamic\n",
            "input":[
                {"role":"user","content":[{"type":"input_text","text":"Original"},{"type":"input_text","text":"追加\n"}]},
                {"type":"message","role":"assistant","id":"msg_1","status":"completed","content":[{"type":"output_text","text":"Let me check\n","annotations":[]}]},
                {"type":"function_call","call_id":"call_original","name":"mcp_read","arguments":raw_arguments},
                {"type":"function_call_output","call_id":"call_original","output":" Complete result ü\r\n"},
                {"role":"user","content":[{"type":"input_text","text":"Continue"}]}
            ],
            "tools":[{"type":"function","name":"mcp_read","description":"Read","parameters":{
                "type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false
            }}], "tool_choice":"auto", "parallel_tool_calls":true,
        })
    );
    // No fictional effective output bound was added to this actual body.
    assert!(body.get("max_output_tokens").is_none());
    assert!(body.get("max_tokens").is_none());
    assert_eq!(observed.technical_headers.len(), 5);
    assert_eq!(observed.technical_headers[ACCEPT], "text/event-stream");
    assert_eq!(observed.technical_headers[CONTENT_TYPE], "application/json");
    assert_eq!(
        observed.technical_headers["openai-beta"],
        "responses=experimental"
    );
    assert_eq!(observed.technical_headers["originator"], "brassclaw");
    assert_eq!(
        observed.technical_headers[USER_AGENT],
        concat!("brassclaw/", env!("CARGO_PKG_VERSION"))
    );
    assert!(observed.technical_headers.get(AUTHORIZATION).is_none());
    assert!(
        observed
            .technical_headers
            .get("chatgpt-account-id")
            .is_none()
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn codex_no_send_corpus_bounds_and_protocol_branches() {
    let config = config("http://127.0.0.1:9/native", "gpt-4o");
    let corpus = codex_prefix_bodies(&config, 5, "Reference\nü", None, true, 65536).unwrap();
    assert_eq!(
        corpus.iter().map(|case| case.id).collect::<Vec<_>>(),
        [
            "single-system",
            "system-join",
            "history",
            "tools-and-history",
            "tool-continuation"
        ]
    );
    let total: usize = corpus.iter().map(|case| case.observation.body.len()).sum();
    assert!(codex_prefix_bodies(&config, 5, "Reference\nü", None, true, total - 1).is_err());
    for case in corpus {
        let body: serde_json::Value = serde_json::from_str(&case.observation.body).unwrap();
        assert!(body.get("messages").is_none());
        assert!(body.get("reasoning").is_none());
        assert!(body.get("include").is_none());
        assert_eq!(case.observation.requested_output_tokens, None);
    }
    let request = CompletionRequest::new(vec![ChatMessage::system(""), ChatMessage::assistant("")]);
    let observed = codex_completion(&config, 5, &request, 4096).unwrap();
    let body: serde_json::Value = serde_json::from_str(&observed.body).unwrap();
    assert_eq!(body["instructions"], "");
    assert_eq!(body["input"][0]["content"][0]["text"], "");
    assert!(codex_completion(&config, 5, &request, 1).is_err());
    assert!(codex_completion(&config, 0, &request, 4096).is_err());
}

#[test]
fn codex_no_send_rejects_unsupported_or_lossy_profiles() {
    let config = config("http://127.0.0.1:9/native", "gpt-5.5");
    let image = ChatMessage::user_with_parts(
        "Image",
        vec![ContentPart::ImageUrl {
            image_url: ImageUrl {
                url: "https://example.invalid/picture".into(),
                detail: None,
            },
        }],
    );
    let mut foreign_reasoning = ChatMessage::assistant("answer");
    foreign_reasoning.reasoning = Some("original reasoning artifact".into());
    for message in [
        image,
        foreign_reasoning,
        ChatMessage::tool_result("foreign", "read", "result"),
    ] {
        let request = CompletionRequest::new(vec![message]);
        assert!(codex_completion(&config, 5, &request, 4096).is_err());
    }
    let call = ToolCall {
        id: "original".into(),
        name: "read".into(),
        arguments: json!({}),
        reasoning: None,
        signature: None,
    };
    let mut signed = call.clone();
    signed.signature = Some("unrepresentable provider artifact".into());
    let mut reasoned = call.clone();
    reasoned.reasoning = Some("unrepresentable reasoning artifact".into());
    for calls in [
        vec![signed],
        vec![reasoned],
        vec![call.clone(), call.clone()],
    ] {
        let request =
            CompletionRequest::new(vec![ChatMessage::assistant_with_tool_calls(None, calls)]);
        assert!(codex_completion(&config, 5, &request, 4096).is_err());
    }
    let mut missing_id = ChatMessage::tool_result("original", "read", "output");
    missing_id.tool_call_id = None;
    let request = CompletionRequest::new(vec![
        ChatMessage::assistant_with_tool_calls(None, vec![call]),
        missing_id,
    ]);
    assert!(codex_completion(&config, 5, &request, 4096).is_err());
    let plain = CompletionRequest::new(vec![ChatMessage::user("Keep all constraints")]);
    let mut requests = vec![];
    let mut request = plain.clone();
    request.temperature = Some(0.5);
    requests.push(request);
    let mut request = plain.clone();
    request.stop_sequences = Some(vec![]);
    requests.push(request);
    let mut request = plain.clone();
    request.model = Some("different".into());
    requests.push(request);
    let mut request = plain.clone();
    request.max_tokens = Some(0);
    requests.push(request);
    for request in requests {
        assert!(codex_completion(&config, 5, &request, 4096).is_err());
    }
    let tool = |name: &str| ToolDefinition {
        name: name.into(),
        description: "read".into(),
        parameters: json!({}),
    };
    let mut request =
        ToolCompletionRequest::from_completion_request(plain, vec![tool("a.b"), tool("a_b")]);
    assert!(codex_tools(&config, 5, &request, 4096).is_err());
    request.tools.pop();
    request.tool_choice = Some("required".into());
    assert!(codex_tools(&config, 5, &request, 4096).is_err());
}

#[tokio::test]
async fn codex_runtime_rejects_unsupported_data_before_auth_or_send() {
    let provider = crate::OpenAiCodexProvider::new_host_with_response_limit(
        "gpt-5.5",
        "http://127.0.0.1:9/native",
        5,
        None,
    )
    .unwrap();
    let mut message = ChatMessage::assistant("answer");
    message.reasoning = Some("unrepresentable original artifact".into());
    let error = provider
        .complete(CompletionRequest::new(vec![message]))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        LlmError::FinalRequestRejected {
            reason: FinalRequestError::Body
        }
    ));
    // A supported request reaches the existing missing-auth boundary. No token,
    // HostSessionSource or false successful model response has been supplied.
    let error = provider
        .complete(CompletionRequest::new(vec![ChatMessage::user("Question")]))
        .await
        .unwrap_err();
    assert!(matches!(error, LlmError::AuthFailed { .. }));
}

#[test]
fn codex_closed_technical_headers_reject_drift_without_credentials() {
    let headers = technical_headers();
    assert!(verify_technical_headers(&headers).is_ok());
    assert!(verify_runtime_headers(&headers).is_err());
    for name in [
        ACCEPT,
        CONTENT_TYPE,
        USER_AGENT,
        HeaderName::from_static("originator"),
        HeaderName::from_static("openai-beta"),
    ] {
        let mut changed = headers.clone();
        changed.remove(&name);
        assert!(verify_technical_headers(&changed).is_err());
        let mut changed = headers.clone();
        changed.insert(name.clone(), HeaderValue::from_static("changed"));
        assert!(verify_technical_headers(&changed).is_err());
        let mut changed = headers.clone();
        changed.append(name, HeaderValue::from_static("duplicate"));
        assert!(verify_technical_headers(&changed).is_err());
    }
    let mut changed = headers;
    changed.insert("x-unqualified", HeaderValue::from_static("unexpected"));
    assert!(verify_technical_headers(&changed).is_err());
}

#[test]
fn codex_endpoint_and_captured_transport_do_not_acquire_chat_qualification() {
    use crate::final_request_transport::{self, FinalRequestProtocol};
    for base in [
        "",
        " https://example.invalid/native",
        "https://u:p@example.invalid/native",
        "https://example.invalid/native?x=1",
        "https://example.invalid/native#frag",
        "file:///native",
        "https://exa\nmple.invalid/native",
    ] {
        assert!(endpoint(base).is_err());
    }
    assert_eq!(
        endpoint("https://example.invalid/backend-api/codex///")
            .unwrap()
            .as_str(),
        "https://example.invalid/backend-api/codex/responses"
    );
    let mut captured = crate::tests::test_llm_config();
    captured.resolved_protocol = Some(crate::ProviderProtocol::OpenAiCodex);
    captured.openai_codex = Some(config(
        "https://example.invalid/backend-api/codex",
        "gpt-5.5",
    ));
    let resolved = final_request_transport::resolve(&captured).unwrap();
    assert!(resolved.protocol == FinalRequestProtocol::CodexResponses);
    assert_eq!(resolved.contract_id(), TRANSPORT);
    assert!(!resolved.matches_contract(
        "brassclaw-rig030-openai-chat/1",
        &crate::final_chat_contract_checksum()
    ));
    assert!(resolved.matches_contract(TRANSPORT, &contract_checksum()));
    captured.openai_codex = None;
    assert!(final_request_transport::resolve(&captured).is_err());
    captured.provider = Some(crate::config::RegistryProviderConfig::generic(
        crate::ProviderProtocol::OpenAiCompletions,
        "chat",
        None,
        "https://example.invalid",
        "chat-model",
    ));
    // A conflicting actual protocol cannot borrow a configured Chat slot.
    assert!(final_request_transport::resolve(&captured).is_err());
    captured.resolved_protocol = Some(crate::ProviderProtocol::OpenAiCompletions);
    let resolved = final_request_transport::resolve(&captured).unwrap();
    assert!(resolved.protocol == FinalRequestProtocol::OpenAiChat);
    assert_eq!(
        resolved.endpoint.as_str(),
        "https://example.invalid/v1/chat/completions"
    );
    assert!(resolved.matches_contract(
        "brassclaw-rig030-openai-chat/1",
        &crate::final_chat_contract_checksum()
    ));
    captured
        .provider
        .as_mut()
        .unwrap()
        .extra_headers
        .push(("x-unqualified".into(), "value".into()));
    assert!(final_request_transport::resolve(&captured).is_err());
}
