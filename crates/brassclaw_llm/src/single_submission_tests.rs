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
