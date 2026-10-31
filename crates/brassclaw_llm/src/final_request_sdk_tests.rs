//! Actual SDK/HTTP boundaries with declared measurement fixtures. These tests
//! qualify transport/custody, not a model/tokenizer/default-template binding.
use super::*;
use crate::{
    ChatMessage, CompletionRequest, LlmProvider, ProviderDispatchPolicy, ToolCompletionRequest,
    ToolDefinition,
};
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
struct Policy(AtomicU64);
#[async_trait]
impl ProviderDispatchGuard for Policy {
    async fn check(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, crate::LlmError> {
        Ok(ProviderDispatchState {
            definition_revision: 1,
            response_cache: Default::default(),
            provider_id: "sdk-provider".into(),
            model: facts.model.into(),
            model_token_limits: Some(crate::ModelTokenLimits {
                max_prefix_tokens: self.0.load(Ordering::SeqCst),
                max_prompt_tokens: 3,
            }),
            configured_context_window_tokens: Some(7),
            requested_output_tokens: facts.max_output_tokens,
        })
    }
}
#[derive(Default)]
struct Wire(Mutex<Vec<Vec<u8>>>);
#[async_trait]
impl FinalRequestMeasurer for Wire {
    async fn measure(
        &self,
        input: FinalRequestInput<'_>,
    ) -> Result<FinalRequestMeasurement, FinalRequestError> {
        // A declared struct field rejects duplicate top-level model keys.
        // A Value map alone would silently retain only the last occurrence.
        #[derive(serde::Deserialize)]
        struct SelectedWireModel {
            model: String,
        }
        let selected: SelectedWireModel =
            serde_json::from_slice(input.body).map_err(|_| FinalRequestError::Measurement)?;
        let body: Value =
            serde_json::from_slice(input.body).map_err(|_| FinalRequestError::Measurement)?;
        if selected.model != input.model {
            return Err(FinalRequestError::Binding);
        }
        let output = body["max_completion_tokens"]
            .as_u64()
            .or_else(|| body["max_tokens"].as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(FinalRequestError::Measurement)?;
        self.0.lock().unwrap().push(input.body.to_vec());
        Ok(FinalRequestMeasurement {
            provider_id: input.provider_id.into(),
            body_sha256: Sha256::digest(input.body).into(),
            model: input.model.into(),
            reserved_output_tokens: output,
            counts: ModelRequestTokenCounts {
                prefix_tokens: 2,
                prompt_tokens: 3,
                input_tokens: 5,
            },
        })
    }
}
fn provider(address: std::net::SocketAddr) -> Arc<dyn LlmProvider> {
    crate::create_registry_provider_inner(
        &crate::config::RegistryProviderConfig::generic(
            crate::registry::ProviderProtocol::OpenAiCompletions,
            "sdk-provider",
            None,
            format!("http://{address}/v1"),
            "sdk-model",
        ),
        5,
    )
    .unwrap()
}
fn guard(
    address: std::net::SocketAddr,
    policy: Arc<Policy>,
    wire: Arc<Wire>,
) -> Arc<FinalRequestGuard> {
    Arc::new(
        FinalRequestGuard::new(
            "sdk-provider".into(),
            "sdk-model".into(),
            format!("http://{address}/v1/chat/completions")
                .parse()
                .unwrap(),
            policy,
            wire,
        )
        .unwrap(),
    )
}
fn request() -> CompletionRequest {
    let mut r = CompletionRequest::new(vec![
        ChatMessage::system("fixed-prefix"),
        ChatMessage::user("history µ\nrequest"),
    ])
    .with_model("sdk-model")
    .with_max_tokens(2);
    r.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
    r
}
async fn once(listener: &tokio::net::TcpListener, status: &str, body: &str) -> Vec<u8> {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut raw = Vec::new();
    let mut block = [0; 4096];
    let bytes = loop {
        let n = socket.read(&mut block).await.unwrap();
        assert!(n > 0);
        raw.extend_from_slice(&block[..n]);
        if let Some(end) = raw.windows(4).position(|p| p == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&raw[..end]).unwrap();
            let len: usize = headers
                .lines()
                .filter_map(|l| l.split_once(':'))
                .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
                .unwrap()
                .1
                .trim()
                .parse()
                .unwrap();
            if raw.len() >= end + 4 + len {
                break raw[end + 4..end + 4 + len].to_vec();
            }
        }
    };
    socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
    bytes
}

#[tokio::test]
async fn sdk_guard_measures_final_tool_schema_and_the_actual_sent_body() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        once(
            &listener,
            "400 Bad Request",
            r#"{"error":{"message":"fixture rejection"}}"#,
        )
        .await
    });
    let p = provider(address);
    let wire = Arc::new(Wire::default());
    let g = guard(address, Arc::new(Policy(AtomicU64::new(2))), wire.clone());
    let tool = ToolDefinition {
        name: "network_probe".into(),
        description: "read one interface".into(),
        parameters: json!({"type":"object","properties":{"iface":{"type":"string"}}}),
    };
    assert!(tool.parameters.get("additionalProperties").is_none());
    let answer = g
        .complete_with_tools(
            p.as_ref(),
            ToolCompletionRequest::from_completion_request(request(), vec![tool]),
        )
        .await;
    assert!(matches!(
        answer,
        Err(crate::LlmError::RequestRejected { status: 400, .. })
    ));
    let sent = upstream.await.unwrap();
    assert_eq!(*wire.0.lock().unwrap(), vec![sent.clone()]);
    let body: Value = serde_json::from_slice(&sent).unwrap();
    assert_eq!(
        body["tools"][0]["function"]["parameters"]["additionalProperties"],
        false
    );
    assert_eq!(
        body["tools"][0]["function"]["parameters"]["required"],
        json!(["iface"])
    );
    assert_eq!(body["max_completion_tokens"], 2);
    assert!(current().is_none());
}

#[tokio::test]
async fn scoped_rejection_bypasses_cache_and_never_retries_or_trips_the_breaker() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let answer = r#"{"id":"fixture","object":"chat.completion","created":0,"model":"sdk-model","choices":[{"index":0,"message":{"role":"assistant","content":"fixture cache entry"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":1,"total_tokens":6}}"#;
        let first = once(&listener, "200 OK", answer).await;
        let second = once(
            &listener,
            "400 Bad Request",
            r#"{"error":{"message":"fixture rejection"}}"#,
        )
        .await;
        vec![first, second]
    });
    let retry: Arc<dyn LlmProvider> = Arc::new(
        crate::RetryProvider::new_with_timing(
            provider(address),
            crate::RetryConfig { max_retries: 3 },
            crate::RetryTimingPolicy {
                base_delay_ms: 1,
                max_backoff_ms: 1,
                jitter_percent: 0,
                max_retry_after_secs: 1,
            },
        )
        .unwrap(),
    );
    let breaker: Arc<dyn LlmProvider> = Arc::new(crate::CircuitBreakerProvider::new(
        retry,
        crate::CircuitBreakerConfig {
            failure_threshold: 1,
            ..Default::default()
        },
    ));
    let cached = crate::CachedProvider::new(
        breaker,
        crate::ResponseCacheConfig {
            ttl: std::time::Duration::from_secs(30),
            max_entries: 2,
        },
    );
    let mut warm = request();
    warm.dispatch_policy = ProviderDispatchPolicy::Configured;
    cached.complete(warm).await.unwrap();
    let policy = Arc::new(Policy(AtomicU64::new(1)));
    let wire = Arc::new(Wire::default());
    let g = guard(address, policy.clone(), wire.clone());
    let mut r = request();
    r.dispatch_policy = ProviderDispatchPolicy::Configured;
    let denied = g.clone().complete(&cached, r).await;
    let Err(error) = denied else {
        panic!("a cached response must not bypass final validation")
    };
    assert!(matches!(
        error,
        crate::LlmError::FinalRequestRejected {
            reason: FinalRequestError::Tokens(ModelTokenCheckError::PrefixExceeded)
        }
    ));
    assert!(!crate::retry::is_retryable(&error));
    assert_eq!(wire.0.lock().unwrap().len(), 1);
    policy.0.store(2, Ordering::SeqCst);
    let answer = g.complete(&cached, request()).await;
    assert!(
        matches!(
            answer,
            Err(crate::LlmError::RequestRejected { status: 400, .. })
        ),
        "local input rejection must not open the breaker"
    );
    assert_eq!(wire.0.lock().unwrap().len(), 2);
    let bodies = server.await.unwrap();
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0], bodies[1]);
    assert!(current().is_none());
}

#[tokio::test]
async fn an_unqualified_adapter_is_rejected_before_any_provider_invocation() {
    struct Unsupported;
    #[async_trait]
    impl LlmProvider for Unsupported {
        fn model_name(&self) -> std::borrow::Cow<'_, str> {
            "sdk-model".into()
        }
        fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
            (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
        }
        async fn complete(
            &self,
            _r: CompletionRequest,
        ) -> Result<crate::CompletionResponse, crate::LlmError> {
            panic!("unsupported provider entered")
        }
        async fn complete_with_tools(
            &self,
            _r: ToolCompletionRequest,
        ) -> Result<crate::ToolCompletionResponse, crate::LlmError> {
            panic!("unsupported provider entered")
        }
    }
    let g = guard(
        "127.0.0.1:1".parse().unwrap(),
        Arc::new(Policy(AtomicU64::new(2))),
        Arc::new(Wire::default()),
    );
    assert!(matches!(
        g.complete(&Unsupported, request()).await,
        Err(crate::LlmError::FinalRequestRejected {
            reason: FinalRequestError::UnsupportedTransport
        })
    ));
}

#[tokio::test]
async fn sdk_size_rejection_records_native_barrier_without_a_submission() {
    use crate::provider_observation::{
        ProviderObservationError, ProviderWireObserver, observe_chat_completion,
    };
    use std::sync::atomic::AtomicBool;
    #[derive(Default)]
    struct Custody {
        started: AtomicBool,
        rejected: AtomicBool,
    }
    #[async_trait]
    impl ProviderWireObserver for Custody {
        async fn before_submission(&self) -> Result<(), ProviderObservationError> {
            Ok(())
        }
        fn submission_started(&self) {
            self.started.store(true, Ordering::SeqCst)
        }
        fn submission_rejected(&self) {
            self.rejected.store(true, Ordering::SeqCst)
        }
        fn receive_response(
            &self,
            _status: u16,
            _body: &[u8],
        ) -> Result<(), ProviderObservationError> {
            panic!("denied request received HTTP response")
        }
        async fn persist_response(&self) -> Result<(), ProviderObservationError> {
            panic!("denied request attempted response persistence")
        }
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let provider = provider(address);
    let custody = Arc::new(Custody::default());
    let g = guard(
        address,
        Arc::new(Policy(AtomicU64::new(1))),
        Arc::new(Wire::default()),
    );
    let result =
        observe_chat_completion(custody.clone(), g.complete(provider.as_ref(), request())).await;
    assert!(matches!(
        result,
        Err(crate::LlmError::FinalRequestRejected {
            reason: FinalRequestError::Tokens(ModelTokenCheckError::PrefixExceeded)
        })
    ));
    assert!(custody.rejected.load(Ordering::SeqCst));
    assert!(!custody.started.load(Ordering::SeqCst));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn guarded_model_override_requires_the_actual_selected_immutable_client() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let provider = provider(address);
    let wire = Arc::new(Wire::default());
    let guard = Arc::new(
        FinalRequestGuard::new(
            "sdk-provider".into(),
            "other-model".into(),
            format!("http://{address}/v1/chat/completions")
                .parse()
                .unwrap(),
            Arc::new(Policy(AtomicU64::new(2))),
            wire.clone(),
        )
        .unwrap(),
    );
    let result = guard
        .complete(provider.as_ref(), request().with_model("other-model"))
        .await;
    assert!(matches!(
        result,
        Err(crate::LlmError::FinalRequestRejected {
            reason: FinalRequestError::Target
        })
    ));
    assert!(wire.0.lock().unwrap().is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
