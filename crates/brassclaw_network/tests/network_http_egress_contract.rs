// Byte buffer sizes for the single-shot local test servers that echo one request.
const CAPTURING_SERVER_READ_BUF: usize = 2048;
const SIMPLE_SERVER_READ_BUF: usize = 1024;

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpListener};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use brassclaw_host_api::{
    InvocationId, NetworkMethod, NetworkPolicy, NetworkTargetPattern, ResourceScope, TenantId,
    UserId,
};
use brassclaw_network::{
    DEFAULT_RESPONSE_BODY_LIMIT, NetworkHttpEgress, NetworkHttpError, NetworkHttpRequest,
    NetworkHttpResponse, NetworkHttpTransport, NetworkResolver, NetworkTransportRequest,
    NetworkUsage, PolicyNetworkHttpEgress, ReqwestNetworkTransport,
};

#[tokio::test]
async fn http_egress_authorizes_default_https_port_and_pins_resolved_ip() {
    let resolved_ips = vec![
        IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
        IpAddr::V4(Ipv4Addr::new(93, 184, 216, 35)),
    ];
    let transport = RecordingTransport::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: b"ok".to_vec(),
        usage: NetworkUsage {
            request_bytes: 0,
            response_bytes: 2,
            resolved_ip: None,
        },
    });
    let requests = transport.requests.clone();
    let egress = PolicyNetworkHttpEgress::new_with_resolver(
        transport,
        StaticResolver::new(resolved_ips.clone()),
    );

    let response = egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Get,
            url: "https://api.example.test/v1".to_string(),
            headers: vec![],
            body: vec![],
            policy: policy("api.example.test", Some(443), true, None),
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect("default HTTPS port should satisfy a 443 policy");

    assert_eq!(response.usage.response_bytes, 2);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].resolved_ips, resolved_ips);
    assert_eq!(requests[0].response_body_limit, Some(1024));
}

#[tokio::test]
async fn http_egress_forwards_timeout_to_transport() {
    let transport = RecordingTransport::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: b"ok".to_vec(),
        usage: NetworkUsage {
            request_bytes: 0,
            response_bytes: 2,
            resolved_ip: None,
        },
    });
    let requests = transport.requests.clone();
    let egress = PolicyNetworkHttpEgress::new_with_resolver(
        transport,
        StaticResolver::new(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]),
    );

    egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Get,
            url: "https://api.example.test/v1".to_string(),
            headers: vec![],
            body: vec![],
            policy: policy("api.example.test", Some(443), true, None),
            response_body_limit: Some(1024),
            timeout_ms: Some(250),
        })
        .await
        .expect("network response should be returned");

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].timeout_ms, Some(250));
}

#[tokio::test]
async fn http_egress_forwards_sensitive_headers_in_zeroizing_transport_carrier() {
    let transport = RecordingTransport::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: b"ok".to_vec(),
        usage: NetworkUsage {
            request_bytes: 0,
            response_bytes: 2,
            resolved_ip: None,
        },
    });
    let requests = transport.requests.clone();
    let egress = PolicyNetworkHttpEgress::new_with_resolver(
        transport,
        StaticResolver::new(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]),
    );

    egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Get,
            url: "https://api.example.test/v1?token=sk-query-secret".to_string(),
            headers: vec![(
                "authorization".to_string(),
                "Bearer sk-header-secret".to_string(),
            )],
            body: vec![],
            policy: policy("api.example.test", Some(443), true, None),
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect("network response should be returned");

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    require_zeroize_on_drop(&requests[0]);
    assert_eq!(
        requests[0].url,
        "https://api.example.test/v1?token=sk-query-secret"
    );
    assert_eq!(
        requests[0]
            .headers
            .iter()
            .find(|(name, _)| name == "authorization"),
        Some(&(
            "authorization".to_string(),
            "Bearer sk-header-secret".to_string()
        ))
    );
}

#[tokio::test]
async fn http_egress_denies_private_resolved_host_before_transport() {
    let transport = RecordingTransport::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: vec![],
        usage: NetworkUsage::default(),
    });
    let requests = transport.requests.clone();
    let egress = PolicyNetworkHttpEgress::new_with_resolver(
        transport,
        StaticResolver::new(vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7))]),
    );

    let error = egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Post,
            url: "https://api.example.test/v1".to_string(),
            headers: vec![],
            body: b"hello".to_vec(),
            policy: policy("api.example.test", Some(443), true, Some(1024)),
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect_err("private resolved targets should fail closed");

    assert!(error.to_string().contains("private"));
    assert_eq!(error.request_bytes(), 50);
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn http_egress_counts_url_and_headers_in_policy_egress_estimate_before_transport() {
    let transport = RecordingTransport::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: vec![],
        usage: NetworkUsage::default(),
    });
    let requests = transport.requests.clone();
    let egress = PolicyNetworkHttpEgress::new_with_resolver(
        transport,
        StaticResolver::new(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]),
    );

    let error = egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Get,
            url: "https://api.example.test/v1/echo?query=large".to_string(),
            headers: vec![("x-trace".to_string(), "abcdef".to_string())],
            body: vec![],
            policy: policy("api.example.test", Some(443), true, Some(4)),
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect_err("URL and headers must count toward egress policy estimates");

    assert!(matches!(error, NetworkHttpError::PolicyDenied { .. }));
    assert!(error.to_string().contains("egress estimate"));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn http_egress_maps_panicking_resolver_to_transport_error() {
    let transport = RecordingTransport::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: b"ok".to_vec(),
        usage: NetworkUsage::default(),
    });
    let requests = transport.requests.clone();
    let egress = PolicyNetworkHttpEgress::new_with_resolver(transport, PanickingResolver);

    let error = egress
        .execute(sample_request("https://api.example.test/v1"))
        .await
        .expect_err("resolver panic should map to a transport error");

    assert!(matches!(error, NetworkHttpError::Transport { .. }));
    assert!(error.to_string().contains("network resolver worker failed"));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn http_egress_rejects_caller_provided_host_header_before_transport() {
    let transport = RecordingTransport::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: vec![],
        usage: NetworkUsage::default(),
    });
    let requests = transport.requests.clone();
    let egress = PolicyNetworkHttpEgress::new_with_resolver(
        transport,
        StaticResolver::new(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]),
    );

    let error = egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Get,
            url: "https://api.example.test/v1".to_string(),
            headers: vec![("Host".to_string(), "evil.example.test".to_string())],
            body: vec![],
            policy: policy("api.example.test", Some(443), true, Some(1024)),
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect_err("caller-provided Host should not be forwarded after URL policy validation");

    assert!(matches!(error, NetworkHttpError::PolicyDenied { .. }));
    assert!(error.to_string().contains("Host header"));
    assert_eq!(error.request_bytes(), 0);
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn http_egress_rejects_userinfo_url_before_transport() {
    let transport = RecordingTransport::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: vec![],
        usage: NetworkUsage::default(),
    });
    let requests = transport.requests.clone();
    let egress = PolicyNetworkHttpEgress::new_with_resolver(
        transport,
        StaticResolver::new(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]),
    );

    let error = egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Get,
            url: "https://user:pass@api.example.test/v1".to_string(),
            headers: vec![],
            body: vec![],
            policy: policy("api.example.test", Some(443), true, Some(1024)),
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect_err("userinfo credentials in URLs must fail before policy/DNS/transport");

    assert!(matches!(error, NetworkHttpError::InvalidUrl { .. }));
    assert!(error.to_string().contains("userinfo"));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn reqwest_transport_does_not_follow_redirects() {
    let (url, server) = single_response_server(
        "HTTP/1.1 302 Found\r\nLocation: http://example.invalid/\r\nContent-Length: 0\r\n\r\n",
    );
    let egress = PolicyNetworkHttpEgress::new(ReqwestNetworkTransport::new(Duration::from_secs(2)));

    let response = egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Get,
            url,
            headers: vec![],
            body: vec![],
            policy: policy("127.0.0.1", None, false, None),
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect("redirect responses should be returned, not followed");
    server.join().unwrap();

    assert_eq!(response.status, 302);
    assert_eq!(response.usage.request_bytes, 0);
    assert_eq!(response.usage.response_bytes, 0);
}

#[tokio::test]
async fn reqwest_transport_forwards_all_headers_to_http_client() {
    let (url, request_bytes, server) =
        captured_request_server("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let transport = ReqwestNetworkTransport::new(Duration::from_secs(2));

    let response = transport
        .execute(NetworkTransportRequest {
            method: NetworkMethod::Get,
            url,
            headers: vec![
                (
                    "authorization".to_string(),
                    "Bearer sk-header-secret".to_string(),
                ),
                (
                    "x-api-key".to_string(),
                    "sk-second-header-secret".to_string(),
                ),
            ],
            body: vec![],
            resolved_ips: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect("transport should forward borrowed header values");
    server.join().unwrap();

    let raw_request = String::from_utf8(request_bytes.lock().unwrap().clone()).unwrap();
    assert_eq!(response.status, 200);
    assert!(raw_request.contains("\r\nauthorization: Bearer sk-header-secret\r\n"));
    assert!(raw_request.contains("\r\nx-api-key: sk-second-header-secret\r\n"));
}

#[tokio::test]
async fn reqwest_transport_uses_all_resolved_addresses_for_connection_fallback() {
    let (url, server) = single_response_server_for_host(
        "fallback.example.test",
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok",
    );
    let transport = ReqwestNetworkTransport::new(Duration::from_secs(2));

    let response = transport
        .execute(NetworkTransportRequest {
            method: NetworkMethod::Get,
            url,
            headers: vec![],
            body: vec![],
            resolved_ips: vec![
                IpAddr::V6(Ipv6Addr::LOCALHOST),
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            ],
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .expect("transport should allow connector fallback across resolved addresses");
    server.join().unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"ok");
}

#[tokio::test]
async fn reqwest_transport_enforces_streaming_response_limit_separately_from_request_bytes() {
    let (url, server) =
        single_response_server("HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nabcdef");
    let egress = PolicyNetworkHttpEgress::new(ReqwestNetworkTransport::new(Duration::from_secs(2)));

    let error = egress
        .execute(NetworkHttpRequest {
            scope: sample_scope(),
            method: NetworkMethod::Post,
            url,
            headers: vec![],
            body: b"hello".to_vec(),
            policy: policy("127.0.0.1", None, false, Some(1024)),
            response_body_limit: Some(5),
            timeout_ms: None,
        })
        .await
        .expect_err("response body limit should stop reads after the limit");
    server.join().unwrap();

    assert!(matches!(
        error,
        NetworkHttpError::ResponseBodyLimit {
            partial_response: Some(_),
            ..
        }
    ));
    assert_eq!(error.request_bytes(), 5);
    assert_eq!(error.response_bytes(), 6);
    let partial_response = error
        .into_partial_response()
        .expect("transport should retain a capped partial response");
    assert_eq!(partial_response.body, b"abcde");
}

#[tokio::test]
async fn reqwest_transport_rejects_invalid_limits_before_dispatch() {
    for (response_body_limit, timeout_ms) in [
        (Some(0), None),
        (Some(u64::MAX), None),
        (Some(1024), Some(0)),
    ] {
        let error = ReqwestNetworkTransport::default()
            .execute(NetworkTransportRequest {
                method: NetworkMethod::Get,
                // No URL parsing or network attempt should precede validation.
                url: "invalid URL containing private-request-data".into(),
                headers: Vec::new(),
                body: b"body".to_vec(),
                resolved_ips: Vec::new(),
                response_body_limit,
                timeout_ms,
            })
            .await
            .unwrap_err();
        assert!(matches!(error, NetworkHttpError::PolicyDenied { .. }));
        assert_eq!(error.request_bytes(), 4);
        assert_eq!(error.response_bytes(), 0);
        assert!(!format!("{error:?}").contains("private-request-data"));
    }
}

#[tokio::test]
async fn reqwest_transport_honors_explicit_timeout_above_default() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; SIMPLE_SERVER_READ_BUF];
        let received = stream.read(&mut request).unwrap();
        assert!(received > 0, "delayed endpoint must receive a request");
        std::thread::sleep(Duration::from_millis(300));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });
    let response = ReqwestNetworkTransport::new(Duration::from_millis(100))
        .execute(NetworkTransportRequest {
            method: NetworkMethod::Get,
            url: format!("http://{address}/test"),
            headers: Vec::new(),
            body: Vec::new(),
            resolved_ips: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            response_body_limit: Some(1024),
            timeout_ms: Some(2000),
        })
        .await
        .unwrap();
    assert_eq!(response.body, b"ok");
    server.join().unwrap();
}

#[tokio::test]
async fn reqwest_transport_honors_explicit_response_limit_above_default() {
    let body_len = DEFAULT_RESPONSE_BODY_LIMIT + 1;
    let (url, server) = sized_response_server(body_len);
    let transport = ReqwestNetworkTransport::new(Duration::from_secs(2));

    let response = transport
        .execute(NetworkTransportRequest {
            method: NetworkMethod::Get,
            url,
            headers: vec![],
            body: vec![],
            resolved_ips: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
            response_body_limit: Some(DEFAULT_RESPONSE_BODY_LIMIT + 1024),
            timeout_ms: None,
        })
        .await
        .expect("an explicit representable limit must not be silently clamped");
    server.join().unwrap();

    assert_eq!(response.usage.response_bytes, body_len);
    assert_eq!(response.body.len() as u64, body_len);
}

#[tokio::test]
async fn reqwest_transport_bounds_unspecified_response_limit_by_default() {
    let body_len = DEFAULT_RESPONSE_BODY_LIMIT + 1;
    let (url, server) = sized_response_server(body_len);
    let transport = ReqwestNetworkTransport::new(Duration::from_secs(2));

    let error = transport
        .execute(NetworkTransportRequest {
            method: NetworkMethod::Get,
            url,
            headers: vec![],
            body: vec![],
            resolved_ips: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
            response_body_limit: None,
            timeout_ms: None,
        })
        .await
        .expect_err("unspecified response limits should still be bounded");
    server.join().unwrap();

    assert!(matches!(
        error,
        NetworkHttpError::ResponseBodyLimit {
            limit: DEFAULT_RESPONSE_BODY_LIMIT,
            partial_response: Some(_),
            ..
        }
    ));
    assert_eq!(error.response_bytes(), DEFAULT_RESPONSE_BODY_LIMIT + 1);
    let partial_response = error
        .into_partial_response()
        .expect("transport should retain a capped partial response");
    assert_eq!(
        partial_response.body.len() as u64,
        DEFAULT_RESPONSE_BODY_LIMIT
    );
}

#[tokio::test]
async fn http_egress_concurrent_requests_do_not_serialize_on_transport() {
    let transport = DelayedTransport::new(Duration::from_millis(100));
    let egress = PolicyNetworkHttpEgress::new_with_resolver(
        transport,
        StaticResolver::new(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]),
    );

    let started = Instant::now();
    let (left, middle, right) = tokio::join!(
        egress.execute(sample_request("https://api.example.test/v1/left")),
        egress.execute(sample_request("https://api.example.test/v1/middle")),
        egress.execute(sample_request("https://api.example.test/v1/right")),
    );

    left.expect("left request should complete");
    middle.expect("middle request should complete");
    right.expect("right request should complete");
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "concurrent async egress calls should overlap instead of serializing"
    );
}

#[derive(Clone)]
struct RecordingTransport {
    response: Result<NetworkHttpResponse, NetworkHttpError>,
    requests: Arc<Mutex<Vec<NetworkTransportRequest>>>,
}

impl RecordingTransport {
    fn ok(response: NetworkHttpResponse) -> Self {
        Self {
            response: Ok(response),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl NetworkHttpTransport for RecordingTransport {
    async fn execute(
        &self,
        request: NetworkTransportRequest,
    ) -> Result<NetworkHttpResponse, NetworkHttpError> {
        self.requests.lock().unwrap().push(request);
        self.response.clone()
    }
}

#[derive(Clone)]
struct DelayedTransport {
    delay: Duration,
}

impl DelayedTransport {
    fn new(delay: Duration) -> Self {
        Self { delay }
    }
}

#[async_trait::async_trait]
impl NetworkHttpTransport for DelayedTransport {
    async fn execute(
        &self,
        _request: NetworkTransportRequest,
    ) -> Result<NetworkHttpResponse, NetworkHttpError> {
        tokio::time::sleep(self.delay).await;
        Ok(NetworkHttpResponse {
            status: 200,
            headers: vec![],
            body: b"ok".to_vec(),
            usage: NetworkUsage {
                request_bytes: 0,
                response_bytes: 2,
                resolved_ip: None,
            },
        })
    }
}

#[derive(Clone)]
struct StaticResolver {
    ips: Vec<IpAddr>,
}

impl StaticResolver {
    fn new(ips: Vec<IpAddr>) -> Self {
        Self { ips }
    }
}

impl NetworkResolver for StaticResolver {
    fn resolve_ips(&self, _host: &str, _port: u16) -> Result<Vec<IpAddr>, NetworkHttpError> {
        Ok(self.ips.clone())
    }
}

#[derive(Clone)]
struct PanickingResolver;

impl NetworkResolver for PanickingResolver {
    fn resolve_ips(&self, _host: &str, _port: u16) -> Result<Vec<IpAddr>, NetworkHttpError> {
        panic!("resolver panic")
    }
}

fn require_zeroize_on_drop<T: ?Sized + zeroize::ZeroizeOnDrop>(_: &T) {}

fn sample_request(url: &str) -> NetworkHttpRequest {
    NetworkHttpRequest {
        scope: sample_scope(),
        method: NetworkMethod::Get,
        url: url.to_string(),
        headers: vec![],
        body: vec![],
        policy: policy("api.example.test", Some(443), true, None),
        response_body_limit: Some(1024),
        timeout_ms: None,
    }
}

fn single_response_server(response: &'static str) -> (String, std::thread::JoinHandle<()>) {
    single_response_server_for_host("127.0.0.1", response)
}

#[tokio::test]
async fn actual_transport_errors_do_not_retain_sensitive_urls_or_backend_data() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_port = listener.local_addr().unwrap().port();
    drop(listener);
    let transport = ReqwestNetworkTransport::new(Duration::from_secs(2));
    // Exercise both send() and response.chunk() errors using actual sockets.
    let (partial_url, partial_server) = single_response_server(
        "HTTP/1.1 200 OK\r\nContent-Length: 64\r\nConnection: close\r\n\r\npartial",
    );
    for url in [
        format!("http://127.0.0.1:{closed_port}/private-path"),
        partial_url,
    ] {
        let error = transport
            .execute(NetworkTransportRequest {
                method: NetworkMethod::Get,
                url: format!("{url}?token=private-query-secret"),
                headers: vec![(
                    "authorization".into(),
                    "Bearer private-header-secret".into(),
                )],
                body: Vec::new(),
                resolved_ips: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
                response_body_limit: Some(1024),
                timeout_ms: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(error, NetworkHttpError::Transport { .. }));
        for text in [error.to_string(), format!("{error:?}")] {
            for private in [
                "private-query-secret",
                "private-header-secret",
                "private-path",
                "127.0.0.1",
                "partial",
            ] {
                assert!(
                    !text.contains(private),
                    "sensitive transport diagnostic: {text}"
                );
            }
        }
        assert_eq!(error.stable_reason(), "transport_failed");
    }
    partial_server.join().unwrap();
}

#[tokio::test]
async fn actual_transport_ignores_ambient_proxy_and_uses_vetted_address() {
    const CHILD: &str = "BRASSCLAW_NETWORK_PROXY_ACCEPTANCE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // A subprocess avoids mutating environment shared by concurrent tests.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = format!("http://{}", listener.local_addr().unwrap());
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "actual_transport_ignores_ambient_proxy_and_uses_vetted_address",
                "--nocapture",
            ])
            .env(CHILD, "1");
        for name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            command.env(name, &proxy);
        }
        for name in ["NO_PROXY", "no_proxy"] {
            command.env(name, "");
        }
        // Keep the proxy port bound with no accept loop; use of the proxy
        // fails the child deadline instead of routing to the vetted server.
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "stdout: {} stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let (url, server) = single_response_server_for_host(
        "vetted-address.example.test",
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
    );
    let response = ReqwestNetworkTransport::new(Duration::from_secs(2))
        .execute(NetworkTransportRequest {
            method: NetworkMethod::Get,
            url,
            headers: Vec::new(),
            body: Vec::new(),
            resolved_ips: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            response_body_limit: Some(1024),
            timeout_ms: None,
        })
        .await
        .unwrap();
    assert_eq!(response.body, b"ok");
    server.join().unwrap();
}

fn captured_request_server(
    response: &'static str,
) -> (String, Arc<Mutex<Vec<u8>>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let request_bytes = Arc::new(Mutex::new(Vec::new()));
    let captured = request_bytes.clone();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; CAPTURING_SERVER_READ_BUF];
        let bytes_read = stream.read(&mut request).unwrap();
        captured
            .lock()
            .unwrap()
            .extend_from_slice(&request[..bytes_read]);
        stream.write_all(response.as_bytes()).unwrap();
    });
    (
        format!("http://127.0.0.1:{port}/test"),
        request_bytes,
        handle,
    )
}

fn single_response_server_for_host(
    host: &'static str,
    response: &'static str,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; SIMPLE_SERVER_READ_BUF];
        let _ = stream.read(&mut request).unwrap();
        stream.write_all(response.as_bytes()).unwrap();
    });
    (format!("http://{host}:{port}/test"), handle)
}

fn sized_response_server(body_len: u64) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; SIMPLE_SERVER_READ_BUF];
        let _ = stream.read(&mut request).unwrap();
        let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {body_len}\r\n\r\n");
        stream.write_all(header.as_bytes()).unwrap();
        let chunk = vec![b'a'; 8192];
        let mut remaining = body_len;
        while remaining > 0 {
            let write_len = remaining.min(chunk.len() as u64) as usize;
            if stream.write_all(&chunk[..write_len]).is_err() {
                break;
            }
            remaining -= write_len as u64;
        }
    });
    (format!("http://127.0.0.1:{port}/test"), handle)
}

fn policy(
    host_pattern: &str,
    port: Option<u16>,
    deny_private_ip_ranges: bool,
    max_egress_bytes: Option<u64>,
) -> NetworkPolicy {
    NetworkPolicy {
        allowed_targets: vec![NetworkTargetPattern {
            scheme: None,
            host_pattern: host_pattern.to_string(),
            port,
        }],
        deny_private_ip_ranges,
        max_egress_bytes,
    }
}

fn sample_scope() -> ResourceScope {
    ResourceScope {
        tenant_id: TenantId::new("tenant1").unwrap(),
        user_id: UserId::new("user1").unwrap(),
        agent_id: None,
        project_id: None,
        thread_id: None,
        invocation_id: InvocationId::new(),
    }
}

#[tokio::test]
async fn live_http_defaults_apply_to_real_requests_and_preserve_accepted_bounds() {
    let transport = ReqwestNetworkTransport::default();
    let control = transport.client_cache_control();
    control
        .publish_policy(0, 1, 8, Duration::from_secs(2), 8)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (accepted, ready) = tokio::sync::oneshot::channel();
    let (release, waiting) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 1024];
        assert!(stream.read(&mut buffer).unwrap() > 0);
        accepted.send(()).unwrap();
        waiting.recv().unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nretained")
            .unwrap();
    });
    let request = |addr: std::net::SocketAddr| NetworkTransportRequest {
        method: NetworkMethod::Get,
        url: format!("http://defaults.example.test:{}/", addr.port()),
        headers: vec![],
        body: vec![],
        resolved_ips: vec![addr.ip()],
        response_body_limit: None,
        timeout_ms: None,
    };
    let running = tokio::spawn({
        let transport = transport.clone();
        let request = request(addr);
        async move { transport.execute(request).await }
    });
    ready.await.unwrap();
    control
        .publish_policy(1, 2, 8, Duration::from_millis(500), 3)
        .unwrap();
    // Exceed the successor timeout while the original accepted request waits.
    tokio::time::sleep(Duration::from_millis(650)).await;
    release.send(()).unwrap();
    assert_eq!(running.await.unwrap().unwrap().body, b"retained");
    server.join().unwrap();
    // The successor's omitted response limit is actually enforced over a socket.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 1024];
        assert!(stream.read(&mut buffer).unwrap() > 0);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nover")
            .unwrap();
    });
    assert!(matches!(
        transport.execute(request(addr)).await,
        Err(NetworkHttpError::ResponseBodyLimit { limit: 3, .. })
    ));
    server.join().unwrap();
    // Omitted timeout also reaches the SDK. No timing-dependent server write.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (release, waiting) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 1024];
        assert!(stream.read(&mut buffer).unwrap() > 0);
        waiting.recv().unwrap();
    });
    let result =
        tokio::time::timeout(Duration::from_secs(2), transport.execute(request(addr))).await;
    release.send(()).unwrap();
    server.join().unwrap();
    assert!(
        matches!(result.unwrap(),Err(NetworkHttpError::Transport { ref reason, .. }) if reason == "request timed out")
    );
    let before = control.status().unwrap();
    assert!(control.publish_policy(2, 3, 8, Duration::ZERO, 3).is_err());
    assert_eq!(control.status().unwrap(), before);
    assert!(
        control
            .publish_policy(2, 2, 8, Duration::from_secs(1), 3)
            .is_err()
    );
    control
        .publish_policy(1, 2, 8, Duration::from_millis(500), 3)
        .unwrap();
}
