//! Real HTTP checks of immutable transport and injected measurement custody.
//! The fixture supplies exact declared counts, not a tokenizer/model oracle.
use super::*;
use crate::{LlmError, ModelTokenLimits};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
struct Policy {
    prefix: AtomicU64,
    revision: AtomicU64,
    reads: AtomicU64,
}
#[async_trait]
impl ProviderDispatchGuard for Policy {
    async fn check(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(ProviderDispatchState {
            definition_revision: self.revision.load(Ordering::SeqCst),
            response_cache: Default::default(),
            provider_id: "fixture-provider".into(),
            model: facts.model.into(),
            model_token_limits: Some(ModelTokenLimits {
                max_prefix_tokens: self.prefix.load(Ordering::SeqCst),
                max_prompt_tokens: 3,
            }),
            configured_context_window_tokens: Some(7),
            requested_output_tokens: facts.max_output_tokens,
        })
    }
}
struct Measurement {
    expected: Vec<u8>,
    observed: Mutex<Vec<Vec<u8>>>,
    invalid_hash: bool,
}
#[async_trait]
impl FinalRequestMeasurer for Measurement {
    async fn measure(
        &self,
        input: FinalRequestInput<'_>,
    ) -> Result<FinalRequestMeasurement, FinalRequestError> {
        assert_eq!(input.provider_id, "fixture-provider");
        assert_eq!(input.model, "fixture-model");
        self.observed.lock().unwrap().push(input.body.to_vec());
        if input.body != self.expected {
            return Err(FinalRequestError::Measurement);
        }
        Ok(FinalRequestMeasurement {
            provider_id: input.provider_id.into(),
            body_sha256: if self.invalid_hash {
                [0; 32]
            } else {
                Sha256::digest(input.body).into()
            },
            model: input.model.into(),
            reserved_output_tokens: 2,
            counts: ModelRequestTokenCounts {
                prefix_tokens: 2,
                prompt_tokens: 3,
                input_tokens: 5,
            },
        })
    }
}
fn fixture(
    endpoint: reqwest::Url,
    invalid_hash: bool,
) -> (FinalRequestGuard, Arc<Policy>, Arc<Measurement>, Vec<u8>) {
    // Include full tool/schema/history/Unicode bytes. The transport must neither
    // normalize these nor validate a separate top-level CompletionRequest DTO.
    let bytes=br#"{"model":"fixture-model","messages":[{"role":"system","content":"PREFIX"},{"role":"user","content":"history: \u00b5\n"}],"tools":[{"type":"function","function":{"name":"router_probe","parameters":{"type":"object","properties":{"iface":{"type":"string"}}}}}],"max_completion_tokens":2}"#.to_vec();
    let policy = Arc::new(Policy {
        prefix: AtomicU64::new(2),
        revision: AtomicU64::new(1),
        reads: AtomicU64::new(0),
    });
    let measurement = Arc::new(Measurement {
        expected: bytes.clone(),
        observed: Mutex::new(Vec::new()),
        invalid_hash,
    });
    let guard = FinalRequestGuard::new(
        "fixture-provider".into(),
        "fixture-model".into(),
        endpoint,
        policy.clone(),
        measurement.clone(),
    )
    .unwrap();
    (guard, policy, measurement, bytes)
}
fn idle_listener() -> std::net::TcpListener {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}
fn no_connection(listener: &std::net::TcpListener) {
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
async fn respond_once(listener: tokio::net::TcpListener, response: Vec<u8>) -> Vec<u8> {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut raw = Vec::new();
    let mut block = [0u8; 4096];
    loop {
        let n = socket.read(&mut block).await.unwrap();
        assert!(n > 0);
        raw.extend_from_slice(&block[..n]);
        if let Some(header_end) = raw.windows(4).position(|v| v == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&raw[..header_end]).unwrap();
            let length: usize = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .unwrap()
                .1
                .trim()
                .parse()
                .unwrap();
            if raw.len() >= header_end + 4 + length {
                let body = raw[header_end + 4..header_end + 4 + length].to_vec();
                socket.write_all(&response).await.unwrap();
                return body;
            }
        }
    }
}

#[tokio::test]
async fn actual_wire_bytes_are_measured_and_sent_once_without_reserialization() {
    let listener = tokio::net::TcpListener::from_std(idle_listener()).unwrap();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    )
    .parse()
    .unwrap();
    let (guard, policy, measurement, body) = fixture(endpoint, false);
    let server = tokio::spawn(respond_once(
        listener,
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec(),
    ));
    let request = reqwest::Client::new()
        .post(guard.endpoint.clone())
        .body(body.clone())
        .build()
        .unwrap();
    let checked = guard.prepare(request).await.unwrap();
    let transport = FinalRequestTransport::new(reqwest::Client::builder().no_proxy()).unwrap();
    assert_eq!(checked.send(&transport).await.unwrap().status(), 200);
    assert_eq!(server.await.unwrap(), body);
    assert_eq!(*measurement.observed.lock().unwrap(), vec![body]);
    assert_eq!(policy.reads.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn changed_caps_and_invalid_measurement_never_open_a_connection() {
    let listener = idle_listener();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    )
    .parse()
    .unwrap();
    let (guard, policy, _, body) = fixture(endpoint, false);
    let request = reqwest::Client::new()
        .post(guard.endpoint.clone())
        .body(body)
        .build()
        .unwrap();
    let checked = guard.prepare(request).await.unwrap();
    policy.prefix.store(1, Ordering::SeqCst);
    policy.revision.store(2, Ordering::SeqCst);
    let transport = FinalRequestTransport::new(reqwest::Client::builder().no_proxy()).unwrap();
    assert!(matches!(
        checked.send(&transport).await,
        Err(FinalRequestSendError::Rejected(FinalRequestError::Tokens(
            ModelTokenCheckError::PrefixExceeded
        )))
    ));
    no_connection(&listener);
    let (guard, _, measurement, body) = fixture(guard.endpoint.clone(), true);
    let request = reqwest::Client::new()
        .post(guard.endpoint.clone())
        .body(body)
        .build()
        .unwrap();
    assert!(matches!(
        guard.prepare(request).await,
        Err(FinalRequestError::Binding)
    ));
    assert_eq!(measurement.observed.lock().unwrap().len(), 1);
    no_connection(&listener);
    let wrong_method = reqwest::Client::new()
        .get(guard.endpoint.clone())
        .build()
        .unwrap();
    assert!(matches!(
        guard.prepare(wrong_method).await,
        Err(FinalRequestError::Target)
    ));
    assert_eq!(measurement.observed.lock().unwrap().len(), 1);
    no_connection(&listener);
}

#[tokio::test]
async fn redirects_cannot_move_a_validated_request_to_another_endpoint() {
    let target = idle_listener();
    let listener = tokio::net::TcpListener::from_std(idle_listener()).unwrap();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    )
    .parse()
    .unwrap();
    let (guard, _, _, body) = fixture(endpoint, false);
    let response=format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",target.local_addr().unwrap()).into_bytes();
    let server = tokio::spawn(respond_once(listener, response));
    let request = reqwest::Client::new()
        .post(guard.endpoint.clone())
        .body(body.clone())
        .build()
        .unwrap();
    let checked = guard.prepare(request).await.unwrap();
    // Even an explicitly permissive caller builder cannot override this rule.
    let transport = FinalRequestTransport::new(
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::limited(10)),
    )
    .unwrap();
    assert_eq!(checked.send(&transport).await.unwrap().status(), 307);
    assert_eq!(server.await.unwrap(), body);
    no_connection(&target);
}

#[tokio::test]
async fn cancelling_measurement_cannot_submit_an_unchecked_request() {
    struct PendingMeasurement(Arc<tokio::sync::Notify>);
    #[async_trait]
    impl FinalRequestMeasurer for PendingMeasurement {
        async fn measure(
            &self,
            _input: FinalRequestInput<'_>,
        ) -> Result<FinalRequestMeasurement, FinalRequestError> {
            self.0.notify_one();
            std::future::pending().await
        }
    }
    let listener = idle_listener();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    )
    .parse()
    .unwrap();
    let (original, policy, _, body) = fixture(endpoint, false);
    let entered = Arc::new(tokio::sync::Notify::new());
    let guard = Arc::new(
        FinalRequestGuard::new(
            "fixture-provider".into(),
            "fixture-model".into(),
            original.endpoint.clone(),
            policy.clone(),
            Arc::new(PendingMeasurement(entered.clone())),
        )
        .unwrap(),
    );
    let request = reqwest::Client::new()
        .post(original.endpoint)
        .body(body)
        .build()
        .unwrap();
    let task = tokio::spawn(async move {
        let _ = guard.prepare(request).await;
    });
    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(policy.reads.load(Ordering::SeqCst), 0);
    no_connection(&listener);
}
