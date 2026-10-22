//! Actual HTTP rejection through the real SDK and decorator stack. No generated
//! answer or success fixture stands in for a provider completion here.
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    ChatMessage, CompletionRequest, FailoverProvider, LlmProvider, ProviderDispatchPolicy,
    RetryConfig, RetryProvider, RetryTimingPolicy, SmartRoutingConfig, SmartRoutingProvider,
    ToolCompletionRequest, config::RegistryProviderConfig, registry::ProviderProtocol,
};

#[tokio::test]
async fn actual_http_single_submission_keeps_primary_and_never_resends_or_fails_over() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let observations = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let recorded = observations.clone();
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = tokio::select! {
                changed = stopped.changed() => {
                    assert!(changed.is_ok());
                    break;
                }
                socket = listener.accept() => socket.unwrap(),
            };
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let (path, body) = loop {
                let count = socket.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0, "request closed before its complete body");
                bytes.extend_from_slice(&buffer[..count]);
                assert!(
                    bytes.len() < 64 * 1024,
                    "fixture expects a small complete request"
                );
                if let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        let path = headers
                            .lines()
                            .next()
                            .unwrap()
                            .split_whitespace()
                            .nth(1)
                            .unwrap()
                            .to_owned();
                        let body: serde_json::Value =
                            serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                        break (path, body);
                    }
                }
            };
            recorded.lock().await.push((path, body));
            let body = r#"{"error":{"message":"upstream completion outcome is unknown"}}"#;
            socket.write_all(format!("HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
    });
    let provider = |path: &str| {
        crate::create_registry_provider(
            &RegistryProviderConfig::generic(
                ProviderProtocol::OpenAiCompletions,
                "selected-provider",
                None,
                format!("http://{address}/{path}"),
                "selected-model",
            ),
            5,
        )
        .unwrap()
    };
    let primary = Arc::new(
        RetryProvider::new_with_timing(
            provider("primary"),
            RetryConfig { max_retries: 3 },
            RetryTimingPolicy {
                base_delay_ms: 1,
                max_backoff_ms: 1,
                jitter_percent: 0,
                max_retry_after_secs: 1,
            },
        )
        .unwrap(),
    );
    let routing = Arc::new(SmartRoutingProvider::new(
        primary,
        provider("cheap"),
        SmartRoutingConfig::default(),
    ));
    let failover = FailoverProvider::new(vec![routing, provider("alternate")]).unwrap();
    let request = |max_tokens| {
        let mut request =
            CompletionRequest::new(vec![ChatMessage::user("hello")]).with_model("selected-model");
        request.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
        request.max_tokens = max_tokens;
        request
    };
    assert!(failover.complete(request(Some(1))).await.is_err());
    assert!(
        failover
            .complete_with_tools(ToolCompletionRequest::from_completion_request(
                request(Some(2)),
                vec![]
            ))
            .await
            .is_err()
    );
    assert!(failover.complete(request(None)).await.is_err());
    assert_eq!(observations.lock().await.len(), 3);
    // The same health counter still applies; a primary in cooldown cannot turn
    // this pinned operation into a call to the alternate provider.
    assert!(failover.complete(request(None)).await.is_err());
    let recorded = observations.lock().await;
    assert_eq!(recorded.len(), 3);
    assert!(recorded.iter().all(
        |(path, body)| path == "/primary/chat/completions" && body["model"] == "selected-model"
    ));
    assert!(
        recorded
            .iter()
            .all(|(_, body)| body.get("dispatch_policy").is_none()),
        "host-only policy never becomes provider prompt/body data"
    );
    assert_eq!(recorded[0].1["max_completion_tokens"], 1);
    assert_eq!(recorded[1].1["max_completion_tokens"], 2);
    assert!(recorded[2].1.get("max_completion_tokens").is_none());
    assert!(
        recorded
            .iter()
            .all(|(_, body)| body.get("max_tokens").is_none())
    );
    drop(recorded);
    stop.send(true).unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn actual_http_wire_custody_is_call_local_and_survives_cancelled_persistence() {
    use crate::provider_observation::{
        ProviderObservationError, ProviderWireObserver, observe_chat_completion,
    };
    #[derive(Default)]
    struct Record {
        value: std::sync::Mutex<Option<(u16, Vec<u8>)>>,
        received: tokio::sync::Notify,
        block: bool,
    }
    #[async_trait::async_trait]
    impl ProviderWireObserver for Record {
        fn receive_response(
            &self,
            status: u16,
            body: &[u8],
        ) -> Result<(), ProviderObservationError> {
            let mut value = self.value.lock().unwrap();
            assert!(value.is_none(), "one original response per operation");
            *value = Some((status, body.to_vec()));
            self.received.notify_one();
            Ok(())
        }
        async fn persist_response(&self) -> Result<(), ProviderObservationError> {
            if self.block {
                std::future::pending::<()>().await;
            }
            Ok(())
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for _ in 0..3 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let body = loop {
                let count = socket.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break serde_json::from_slice::<serde_json::Value>(
                            &bytes[end + 4..end + 4 + length],
                        )
                        .unwrap();
                    }
                }
            };
            let mut response = body["messages"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .as_bytes()
                .to_vec();
            response.extend_from_slice(&[0, 255]); // Preserve invalid UTF-8 exactly.
            socket.write_all(format!("HTTP/1.1 503 Service Unavailable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).as_bytes()).await.unwrap();
            socket.write_all(&response).await.unwrap();
        }
    });
    let provider = crate::create_registry_provider(
        &RegistryProviderConfig::generic(
            ProviderProtocol::OpenAiCompletions,
            "wire-provider",
            None,
            format!("http://{address}"),
            "wire-model",
        ),
        5,
    )
    .unwrap();
    let first = Arc::new(Record::default());
    let second = Arc::new(Record::default());
    let (a, b) = tokio::join!(
        observe_chat_completion(
            first.clone(),
            provider.complete(CompletionRequest::new(vec![ChatMessage::user("first")]))
        ),
        observe_chat_completion(
            second.clone(),
            provider.complete(CompletionRequest::new(vec![ChatMessage::user("second")]))
        ),
    );
    assert!(a.is_err() && b.is_err());
    assert_eq!(
        *first.value.lock().unwrap(),
        Some((503, b"first\0\xff".to_vec()))
    );
    assert_eq!(
        *second.value.lock().unwrap(),
        Some((503, b"second\0\xff".to_vec()))
    );
    let cancelled = Arc::new(Record {
        block: true,
        ..Default::default()
    });
    let retained = cancelled.clone();
    let pending = tokio::spawn(async move {
        observe_chat_completion(
            retained,
            provider.complete(CompletionRequest::new(vec![ChatMessage::user("cancelled")])),
        )
        .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        cancelled.received.notified(),
    )
    .await
    .unwrap();
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert_eq!(
        *cancelled.value.lock().unwrap(),
        Some((503, b"cancelled\0\xff".to_vec()))
    );
    assert!(crate::provider_observation::current().is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn actual_registry_transports_preserve_explicit_request_parameters() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn check<T: std::fmt::Debug>(
        result: Result<T, crate::LlmError>,
        received: &AtomicUsize,
        expected: usize,
        protocol: ProviderProtocol,
    ) {
        assert_eq!(
            received.load(Ordering::Acquire),
            expected,
            "{protocol:?} did not send the expected request before returning: {result:?}"
        );
        assert!(
            result.is_err(),
            "the real loopback server rejects every inference"
        );
    }
    for protocol in [
        ProviderProtocol::OpenAiCompletions,
        ProviderProtocol::Anthropic,
        ProviderProtocol::Ollama,
        ProviderProtocol::DeepSeek,
        ProviderProtocol::OpenRouter,
        ProviderProtocol::Gemini,
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let received = Arc::new(AtomicUsize::new(0));
        let captured = received.clone();
        let expected_http = if protocol == ProviderProtocol::Anthropic {
            2
        } else {
            4
        };
        let mut server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for _ in 0..expected_http {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                let body = loop {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert_ne!(count, 0, "SDK must send its full request");
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        if bytes.len() >= end + 4 + length {
                            break serde_json::from_slice::<serde_json::Value>(
                                &bytes[end + 4..end + 4 + length],
                            )
                            .unwrap();
                        }
                    }
                };
                requests.push(body);
                captured.fetch_add(1, Ordering::Release);
                let rejected = b"{\"error\":{\"message\":\"fixture rejects inference\"}}";
                socket.write_all(format!("HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",rejected.len()).as_bytes()).await.unwrap();
                socket.write_all(rejected).await.unwrap();
            }
            requests
        });
        // Non-secret fixture credential only, used with this loopback listener.
        let mut config = RegistryProviderConfig::generic(
            protocol,
            "parameter-contract",
            Some(secrecy::SecretString::from("fixture-key")),
            format!("http://{address}/contract"),
            "model-fixture",
        );
        let provider = crate::create_registry_provider(&config, 5).unwrap();
        let request = |limit, temperature, stops| {
            let mut value = CompletionRequest::new(vec![ChatMessage::user("wire contract")]);
            value.max_tokens = limit;
            value.temperature = temperature;
            value.stop_sequences = stops;
            value
        };
        check(
            provider
                .complete(request(
                    Some(17),
                    Some(0.25),
                    Some(vec!["END".into(), "ü".into()]),
                ))
                .await,
            &received,
            1,
            protocol,
        );
        check(
            provider
                .complete_with_tools(ToolCompletionRequest::from_completion_request(
                    request(Some(18), Some(0.5), Some(vec!["DONE".into()])),
                    vec![],
                ))
                .await,
            &received,
            2,
            protocol,
        );
        let omitted = provider.complete(request(None, None, None)).await;
        if protocol == ProviderProtocol::Anthropic {
            assert!(
                omitted
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("`max_tokens` must be set for Anthropic")
            );
        }
        check(
            omitted,
            &received,
            if protocol == ProviderProtocol::Anthropic {
                2
            } else {
                3
            },
            protocol,
        );
        config.unsupported_params = vec![
            "max_tokens".into(),
            "temperature".into(),
            "stop_sequences".into(),
        ];
        let unsupported = crate::create_registry_provider(&config, 5).unwrap();
        let stripped = unsupported
            .complete(request(Some(17), Some(0.25), Some(vec!["END".into()])))
            .await;
        if protocol == ProviderProtocol::Anthropic {
            assert!(
                stripped
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("`max_tokens` must be set for Anthropic")
            );
        }
        check(stripped, &received, expected_http, protocol);
        let rows = match tokio::time::timeout(std::time::Duration::from_secs(5), &mut server).await
        {
            Ok(result) => result.unwrap(),
            Err(_) => {
                server.abort();
                panic!("{protocol:?} fixture did not finish after its expected complete requests");
            }
        };

        assert_eq!(rows.len(), expected_http);
        let (limit_path, temp_path, stop_path) = match protocol {
            ProviderProtocol::OpenAiCompletions => {
                ("/max_completion_tokens", "/temperature", "/stop")
            }
            ProviderProtocol::Anthropic => ("/max_tokens", "/temperature", "/stop_sequences"),
            ProviderProtocol::Ollama => (
                "/options/num_predict",
                "/options/temperature",
                "/options/stop",
            ),
            ProviderProtocol::DeepSeek | ProviderProtocol::OpenRouter => {
                ("/max_tokens", "/temperature", "/stop")
            }
            ProviderProtocol::Gemini => (
                "/generationConfig/maxOutputTokens",
                "/generationConfig/temperature",
                "/generationConfig/stopSequences",
            ),
            _ => unreachable!("only the six explicit SDK transports are tested"),
        };
        assert_eq!(
            rows[0].pointer(limit_path),
            Some(&serde_json::json!(17)),
            "{protocol:?}"
        );
        assert_eq!(
            rows[1].pointer(limit_path),
            Some(&serde_json::json!(18)),
            "{protocol:?}"
        );
        assert_eq!(
            rows[0].pointer(temp_path),
            Some(&serde_json::json!(0.25)),
            "{protocol:?}"
        );
        assert_eq!(
            rows[1].pointer(temp_path),
            Some(&serde_json::json!(0.5)),
            "{protocol:?}"
        );
        assert_eq!(
            rows[0].pointer(stop_path),
            Some(&serde_json::json!(["END", "ü"])),
            "{protocol:?}"
        );
        assert_eq!(
            rows[1].pointer(stop_path),
            Some(&serde_json::json!(["DONE"])),
            "{protocol:?}"
        );
        for row in &rows[2..] {
            assert!(
                row.pointer(temp_path)
                    .is_none_or(serde_json::Value::is_null),
                "{protocol:?}"
            );
            assert!(
                row.pointer(stop_path)
                    .is_none_or(serde_json::Value::is_null),
                "{protocol:?}"
            );
            assert!(
                row.pointer(limit_path)
                    .is_none_or(serde_json::Value::is_null),
                "{protocol:?}"
            );
        }
        if protocol == ProviderProtocol::Ollama {
            assert!(rows.iter().all(|row| row.get("max_tokens").is_none()));
        }
    }
}
