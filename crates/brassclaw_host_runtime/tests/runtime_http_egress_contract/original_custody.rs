//! Pipeline ordering/normal-output tests, NOT native Auth/issuer acceptance.
//! The native owner has independent actual HTTP + PostgreSQL custody cases.
use super::*;
use brassclaw_host_runtime::{
    HostHttpOriginalCustody, HostHttpOriginalCustodyError, HostHttpOriginalEgress,
};
use secrecy::ExposeSecret;

struct InspectCustodian {
    expected: RuntimeHttpEgressRequest,
    original: Arc<Mutex<Option<SecretMaterial>>>,
    preflights: Arc<std::sync::atomic::AtomicUsize>,
    reject_preflight: bool,
    reject_retain: bool,
}
#[async_trait::async_trait]
impl HostHttpOriginalCustody for InspectCustodian {
    async fn authenticate_request(
        &mut self,
        request: &RuntimeHttpEgressRequest,
    ) -> Result<(), HostHttpOriginalCustodyError> {
        self.preflights
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.reject_preflight || *request != self.expected {
            return Err(HostHttpOriginalCustodyError);
        }
        Ok(())
    }
    async fn retain(
        self: Box<Self>,
        material: SecretMaterial,
    ) -> Result<(), HostHttpOriginalCustodyError> {
        if self.reject_retain {
            return Err(HostHttpOriginalCustodyError);
        }
        *self.original.lock().unwrap() = Some(material);
        Ok(())
    }
}
fn request() -> RuntimeHttpEgressRequest {
    RuntimeHttpEgressRequest {
        runtime: RuntimeKind::FirstParty,
        scope: sample_scope(),
        capability_id: sample_capability_id(),
        method: NetworkMethod::Post,
        url: "https://api.example.test/oauth/token".into(),
        headers: vec![],
        body: b"actual request".to_vec(),
        network_policy: sample_policy(),
        credential_injections: vec![],
        response_body_limit: Some(4096),
        save_body_to: None,
        timeout_ms: Some(5000),
    }
}
fn wire(body: Vec<u8>) -> NetworkHttpResponse {
    NetworkHttpResponse {
        status: 201,
        headers: vec![
            ("x-repeat".into(), "first".into()),
            ("set-cookie".into(), "private-cookie".into()),
            ("x-repeat".into(), "second".into()),
        ],
        usage: NetworkUsage {
            request_bytes: 14,
            response_bytes: body.len() as u64,
            resolved_ip: Some("203.0.113.7".parse().unwrap()),
        },
        body,
    }
}
#[tokio::test]
async fn original_custody_preserves_binary_ordered_headers_and_full_usage_before_normal_sanitizing()
{
    let raw = wire(vec![0, 255, 128, b'a']);
    let network = RecordingNetwork::ok(raw.clone());
    let calls = network.requests.clone();
    let services = test_obligation_services();
    let request = request();
    stage_policy(
        &services,
        &request.scope,
        &request.capability_id,
        sample_policy(),
    )
    .await;
    let original = Arc::new(Mutex::new(None));
    let preflights = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let response = services
        .host_http_egress(network)
        .execute_with_original_custody(
            request.clone(),
            Box::new(InspectCustodian {
                expected: request,
                original: original.clone(),
                preflights: preflights.clone(),
                reject_preflight: false,
                reject_retain: false,
            }),
        )
        .await
        .unwrap();
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(preflights.load(std::sync::atomic::Ordering::SeqCst), 1);
    let retained = original.lock().unwrap();
    let retained: Value = serde_json::from_str(retained.as_ref().unwrap().expose_secret()).unwrap();
    assert_eq!(
        retained,
        json!({"status":raw.status,"headers":raw.headers,"body":raw.body,
        "usage":{"request_bytes":14,"response_bytes":4,"resolved_ip":"203.0.113.7"}})
    );
    assert_eq!(
        response.headers,
        vec![
            ("x-repeat".to_string(), "first".to_string()),
            ("x-repeat".to_string(), "second".to_string())
        ]
    );
    assert_eq!(response.body, vec![0, 255, 128, b'a']);
    assert!(response.redaction_applied);
}
#[tokio::test]
async fn original_custody_preflight_and_retain_faults_bound_actual_transport_count() {
    for reject_preflight in [true, false] {
        let network = RecordingNetwork::ok(wire(b"plain".to_vec()));
        let calls = network.requests.clone();
        let services = test_obligation_services();
        let request = request();
        stage_policy(
            &services,
            &request.scope,
            &request.capability_id,
            sample_policy(),
        )
        .await;
        let original = Arc::new(Mutex::new(None));
        let preflights = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let error = services
            .host_http_egress(network)
            .execute_with_original_custody(
                request.clone(),
                Box::new(InspectCustodian {
                    expected: request,
                    original: original.clone(),
                    preflights: preflights.clone(),
                    reject_preflight,
                    reject_retain: true,
                }),
            )
            .await
            .unwrap_err();
        assert_eq!(calls.lock().unwrap().len(), usize::from(!reject_preflight));
        assert_eq!(preflights.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(original.lock().unwrap().is_none());
        assert_eq!(error.request_bytes(), if reject_preflight { 0 } else { 14 });
    }
}
#[tokio::test]
async fn original_custody_survives_actual_sanitizer_failure_without_second_dispatch() {
    let raw = wire(b"sk-abcdefghijklmnopqrstuvwxyz123456".to_vec());
    let network = RecordingNetwork::ok(raw.clone());
    let calls = network.requests.clone();
    let services = test_obligation_services();
    let request = request();
    stage_policy(
        &services,
        &request.scope,
        &request.capability_id,
        sample_policy(),
    )
    .await;
    let original = Arc::new(Mutex::new(None));
    let error = services
        .host_http_egress(network)
        .execute_with_original_custody(
            request.clone(),
            Box::new(InspectCustodian {
                expected: request,
                original: original.clone(),
                preflights: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                reject_preflight: false,
                reject_retain: false,
            }),
        )
        .await
        .unwrap_err();
    assert_eq!(error.stable_runtime_reason(), "response_leak_blocked");
    assert_eq!(calls.lock().unwrap().len(), 1);
    let retained = original.lock().unwrap();
    let value: Value = serde_json::from_str(retained.as_ref().unwrap().expose_secret()).unwrap();
    assert_eq!(value["body"], json!(raw.body));
}
#[tokio::test]
async fn original_custody_cannot_bypass_missing_staged_policy_or_existing_body_bound() {
    for missing_policy in [true, false] {
        let network = RecordingNetwork::ok(wire(b"oversized".to_vec()));
        let calls = network.requests.clone();
        let services = test_obligation_services();
        let mut request = request();
        request.response_body_limit = Some(4);
        if !missing_policy {
            stage_policy(
                &services,
                &request.scope,
                &request.capability_id,
                sample_policy(),
            )
            .await;
        }
        let original = Arc::new(Mutex::new(None));
        let preflights = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        assert!(
            services
                .host_http_egress(network)
                .execute_with_original_custody(
                    request.clone(),
                    Box::new(InspectCustodian {
                        expected: request,
                        original: original.clone(),
                        preflights: preflights.clone(),
                        reject_preflight: false,
                        reject_retain: false,
                    })
                )
                .await
                .is_err()
        );
        assert_eq!(calls.lock().unwrap().len(), usize::from(!missing_policy));
        assert!(original.lock().unwrap().is_none());
        assert_eq!(
            preflights.load(std::sync::atomic::Ordering::SeqCst),
            usize::from(!missing_policy)
        );
    }
}
