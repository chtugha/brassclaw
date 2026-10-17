use std::{
    collections::hash_map::RandomState,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use brassclaw_host_api::NetworkMethod;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    egress::NetworkHttpTransport,
    error::NetworkHttpError,
    types::{
        DEFAULT_RESPONSE_BODY_LIMIT, NetworkHttpResponse, NetworkTransportRequest, NetworkUsage,
    },
};

#[derive(Clone)]
pub struct ReqwestNetworkTransport {
    client_cache: Arc<Mutex<ClientCache>>,
}

struct ClientCache {
    revision: u64,
    capacity: u32,
    default_timeout: Duration,
    default_response_body_bytes: u64,
    // Capacity is a ceiling, never an up-front allocation. Use a randomized
    // hasher for network-selected keys even when another crate enables lru's
    // optional hashbrown/default-hasher feature.
    entries: lru::LruCache<ReqwestClientKey, reqwest::Client, RandomState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpClientCacheStatus {
    pub revision: u64,
    pub capacity: u32,
    pub retained_entries: usize,
    pub default_timeout: Duration,
    pub default_response_body_bytes: u64,
}

/// Resource control for the same cache used by the hardened transport. It
/// exposes no request execution, policy grants, credentials or client handles.
#[derive(Clone)]
pub struct HttpClientCacheControl {
    transport: ReqwestNetworkTransport,
}
impl HttpClientCacheControl {
    pub fn validate_defaults(
        timeout: Duration,
        response_body_bytes: u64,
    ) -> Result<(), NetworkHttpError> {
        effective_request_timeout(None, timeout, 0)?;
        effective_response_body_limit(None, response_body_bytes, 0)?;
        Ok(())
    }
    pub fn publish_policy(
        &self,
        expected: u64,
        revision: u64,
        capacity: u32,
        timeout: Duration,
        response_body_bytes: u64,
    ) -> Result<(), NetworkHttpError> {
        Self::validate_defaults(timeout, response_body_bytes)?;
        self.transport.publish_client_policy(
            expected,
            revision,
            capacity,
            timeout,
            response_body_bytes,
        )
    }
    pub fn status(&self) -> Result<HttpClientCacheStatus, NetworkHttpError> {
        self.transport.client_cache_status()
    }
    pub fn publish_capacity(
        &self,
        expected: u64,
        revision: u64,
        capacity: u32,
    ) -> Result<(), NetworkHttpError> {
        self.transport
            .publish_client_cache_capacity(expected, revision, capacity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ReqwestClientKey {
    host: String,
    port: u16,
    resolved_addrs: Vec<SocketAddr>,
    timeout: Duration,
}

impl std::fmt::Debug for ReqwestNetworkTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReqwestNetworkTransport")
            .finish_non_exhaustive()
    }
}

impl Default for ReqwestNetworkTransport {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

impl ReqwestNetworkTransport {
    pub fn client_cache_control(&self) -> HttpClientCacheControl {
        HttpClientCacheControl {
            transport: self.clone(),
        }
    }
    pub fn new(timeout: Duration) -> Self {
        Self {
            client_cache: Arc::new(Mutex::new(ClientCache {
                revision: 0,
                capacity: 128,
                default_timeout: timeout,
                default_response_body_bytes: DEFAULT_RESPONSE_BODY_LIMIT,
                entries: lru::LruCache::unbounded_with_hasher(RandomState::new()),
            })),
        }
    }

    pub fn client_cache_status(&self) -> Result<HttpClientCacheStatus, NetworkHttpError> {
        let cache = self.client_cache.lock().map_err(|_| cache_unavailable(0))?;
        Ok(HttpClientCacheStatus {
            revision: cache.revision,
            capacity: cache.capacity,
            retained_entries: cache.entries.len(),
            default_timeout: cache.default_timeout,
            default_response_body_bytes: cache.default_response_body_bytes,
        })
    }

    /// Owner-only settings publication. Shrinking drops least-recent cache
    /// ownership, not handles already captured by active requests. A build
    /// admitted under an older revision cannot repopulate this successor.
    pub fn publish_client_cache_capacity(
        &self,
        expected_revision: u64,
        revision: u64,
        capacity: u32,
    ) -> Result<(), NetworkHttpError> {
        let defaults = self.client_cache_status()?;
        self.publish_client_policy(
            expected_revision,
            revision,
            capacity,
            defaults.default_timeout,
            defaults.default_response_body_bytes,
        )
    }

    fn publish_client_policy(
        &self,
        expected_revision: u64,
        revision: u64,
        capacity: u32,
        timeout: Duration,
        response_body_bytes: u64,
    ) -> Result<(), NetworkHttpError> {
        HttpClientCacheControl::validate_defaults(timeout, response_body_bytes)?;
        let mut cache = self.client_cache.lock().map_err(|_| cache_unavailable(0))?;
        if cache.revision == revision
            && cache.capacity == capacity
            && cache.default_timeout == timeout
            && cache.default_response_body_bytes == response_body_bytes
        {
            return Ok(());
        }
        if cache.revision != expected_revision || revision <= cache.revision {
            return Err(NetworkHttpError::PolicyDenied {
                reason: "HTTP client cache revision conflict".into(),
                request_bytes: 0,
                response_bytes: 0,
            });
        }
        cache.revision = revision;
        cache.capacity = capacity;
        cache.default_timeout = timeout;
        cache.default_response_body_bytes = response_body_bytes;
        while cache.entries.len() > capacity as usize {
            cache.entries.pop_lru();
        }
        Ok(())
    }

    async fn client_for(
        &self,
        key: ReqwestClientKey,
        request_bytes: u64,
    ) -> Result<reqwest::Client, NetworkHttpError> {
        let revision = {
            let mut cache = self
                .client_cache
                .lock()
                .map_err(|_| NetworkHttpError::Transport {
                    reason: "reqwest client cache lock poisoned".to_string(),
                    request_bytes,
                    response_bytes: 0,
                })?;
            if let Some(client) = cache.entries.get(&key).cloned() {
                return Ok(client);
            }
            cache.revision
        };

        let build_key = key.clone();
        let client = tokio::task::spawn_blocking(move || build_reqwest_client(&build_key))
            .await
            .map_err(|error| NetworkHttpError::Transport {
                reason: if error.is_panic() {
                    "reqwest client builder task panicked"
                } else {
                    "reqwest client builder task was cancelled"
                }
                .to_string(),
                request_bytes,
                response_bytes: 0,
            })?
            .map_err(|error| NetworkHttpError::Transport {
                reason: reqwest_error_diagnostic(&error),
                request_bytes,
                response_bytes: 0,
            })?;

        let mut cache = self
            .client_cache
            .lock()
            .map_err(|_| NetworkHttpError::Transport {
                reason: "reqwest client cache lock poisoned".to_string(),
                request_bytes,
                response_bytes: 0,
            })?;
        if cache.revision != revision || cache.capacity == 0 {
            return Ok(client);
        }
        if let Some(existing) = cache.entries.get(&key).cloned() {
            return Ok(existing);
        }
        while cache.entries.len() >= cache.capacity as usize {
            cache.entries.pop_lru();
        }
        cache.entries.put(key, client.clone());
        Ok(client)
    }
}

fn cache_unavailable(request_bytes: u64) -> NetworkHttpError {
    NetworkHttpError::Transport {
        reason: "HTTP client cache unavailable".into(),
        request_bytes,
        response_bytes: 0,
    }
}

fn build_reqwest_client(key: &ReqwestClientKey) -> Result<reqwest::Client, reqwest::Error> {
    let mut builder = reqwest::Client::builder()
        // Ambient proxies would receive the request instead of the vetted DNS
        // addresses and can expose injected credentials to another endpoint.
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(key.timeout);
    if !key.resolved_addrs.is_empty() {
        builder = builder.resolve_to_addrs(&key.host, &key.resolved_addrs);
    }
    builder.build()
}

/// Parses the URL while scrubbing only the source request carrier copy.
///
/// The returned `url::Url` and reqwest internals still retain plaintext while
/// the request is dispatched. Parse failures use a fixed diagnostic rather
/// than `url::ParseError` formatting because those diagnostics may include
/// raw input that contains injected credentials.
/// The source request URL is consumed even on parse failure so callers cannot
/// later inspect a credential-bearing raw URL for diagnostics.
fn take_request_url(
    request: &mut NetworkTransportRequest,
    request_bytes: u64,
) -> Result<url::Url, NetworkHttpError> {
    let mut raw_url = std::mem::take(&mut request.url);
    let parsed = url::Url::parse(&raw_url).map_err(|_| NetworkHttpError::InvalidUrl {
        reason: "URL parse error: invalid format".to_string(),
        request_bytes,
        response_bytes: 0,
    });
    raw_url.zeroize();
    parsed
}

fn apply_request_headers(
    mut req: reqwest::RequestBuilder,
    headers: &mut [(String, String)],
) -> reqwest::RequestBuilder {
    for (name, value) in headers.iter() {
        req = req.header(name.as_str(), value.as_str());
    }
    for (name, value) in headers.iter_mut() {
        name.zeroize();
        value.zeroize();
    }
    // reqwest's internal HeaderMap retains its copied values until the request
    // builder is consumed and the response future completes.
    req
}

#[async_trait]
impl NetworkHttpTransport for ReqwestNetworkTransport {
    async fn execute(
        &self,
        mut request: NetworkTransportRequest,
    ) -> Result<NetworkHttpResponse, NetworkHttpError> {
        let request_bytes = request.body.len() as u64;
        // Capture one coherent policy before any asynchronous work. Accepted
        // requests retain these defaults across settings publication.
        let policy = self.client_cache_status()?;
        let limit = effective_response_body_limit(
            request.response_body_limit,
            policy.default_response_body_bytes,
            request_bytes,
        )?;
        let timeout =
            effective_request_timeout(request.timeout_ms, policy.default_timeout, request_bytes)?;
        reject_caller_host_header(&request.headers)?;
        let url = take_request_url(&mut request, request_bytes)?;
        let host = url
            .host_str()
            .ok_or_else(|| NetworkHttpError::InvalidUrl {
                reason: "URL host is required".to_string(),
                request_bytes,
                response_bytes: 0,
            })?
            .to_string();
        let port = url
            .port_or_known_default()
            .ok_or_else(|| NetworkHttpError::InvalidUrl {
                reason: "URL port is required".to_string(),
                request_bytes,
                response_bytes: 0,
            })?;

        let resolved_addrs = request
            .resolved_ips
            .iter()
            .copied()
            .map(|resolved_ip| SocketAddr::new(resolved_ip, port))
            .collect::<Vec<_>>();
        let client = self
            .client_for(
                ReqwestClientKey {
                    host,
                    port,
                    resolved_addrs,
                    timeout,
                },
                request_bytes,
            )
            .await?;

        let mut headers = Zeroizing::new(std::mem::take(&mut request.headers));
        let mut req = client
            .request(reqwest_method(request.method), url)
            .body(std::mem::take(&mut request.body));
        req = apply_request_headers(req, &mut headers[..]);
        let mut response = req
            .send()
            .await
            .map_err(|error| NetworkHttpError::Transport {
                reason: reqwest_error_diagnostic(&error),
                request_bytes,
                response_bytes: 0,
            })?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| Some((name.to_string(), value.to_str().ok()?.to_string())))
            .collect::<Vec<_>>();
        let mut body = Vec::new();
        while let Some(chunk) =
            response
                .chunk()
                .await
                .map_err(|error| NetworkHttpError::Transport {
                    reason: reqwest_error_diagnostic(&error),
                    request_bytes,
                    response_bytes: body.len() as u64,
                })?
        {
            let current_len = body.len() as u64;
            let remaining = limit.saturating_sub(current_len);
            if chunk.len() as u64 > remaining {
                let take = remaining as usize;
                reserve_response_bytes(&mut body, take, request_bytes)?;
                body.extend_from_slice(&chunk[..take]);
                return Err(NetworkHttpError::ResponseBodyLimit {
                    limit,
                    request_bytes,
                    response_bytes: limit.saturating_add(1),
                    partial_response: Some(NetworkHttpResponse {
                        status,
                        headers,
                        body,
                        usage: NetworkUsage {
                            request_bytes,
                            response_bytes: limit.saturating_add(1),
                            resolved_ip: request.resolved_ips.first().copied(),
                        },
                    }),
                });
            }
            reserve_response_bytes(&mut body, chunk.len(), request_bytes)?;
            body.extend_from_slice(&chunk);
        }
        let response_bytes = body.len() as u64;
        Ok(NetworkHttpResponse {
            status,
            headers,
            body,
            usage: NetworkUsage {
                request_bytes,
                response_bytes,
                resolved_ip: request.resolved_ips.first().copied(),
            },
        })
    }
}

fn effective_response_body_limit(
    requested: Option<u64>,
    default: u64,
    request_bytes: u64,
) -> Result<u64, NetworkHttpError> {
    let limit = requested.unwrap_or(default);
    if limit == 0 || limit > isize::MAX as u64 {
        return Err(NetworkHttpError::PolicyDenied {
            reason: "response body limit must be a positive representable allocation size".into(),
            request_bytes,
            response_bytes: 0,
        });
    }
    Ok(limit)
}

fn effective_request_timeout(
    requested_ms: Option<u32>,
    default: Duration,
    request_bytes: u64,
) -> Result<Duration, NetworkHttpError> {
    let timeout = requested_ms
        .map(|millis| Duration::from_millis(u64::from(millis)))
        .unwrap_or(default);
    if timeout.is_zero() || std::time::Instant::now().checked_add(timeout).is_none() {
        return Err(NetworkHttpError::PolicyDenied {
            reason: "request timeout must be positive and representable on this host".into(),
            request_bytes,
            response_bytes: 0,
        });
    }
    Ok(timeout)
}

fn reserve_response_bytes(
    body: &mut Vec<u8>,
    additional: usize,
    request_bytes: u64,
) -> Result<(), NetworkHttpError> {
    body.try_reserve(additional)
        .map_err(|_| NetworkHttpError::Transport {
            reason: "response buffer allocation failed".into(),
            request_bytes,
            response_bytes: body.len() as u64,
        })
}

pub(crate) fn reject_caller_host_header(
    headers: &[(String, String)],
) -> Result<(), NetworkHttpError> {
    if headers
        .iter()
        .any(|(name, _)| name.trim().eq_ignore_ascii_case("host"))
    {
        return Err(NetworkHttpError::PolicyDenied {
            reason: "caller-provided Host header is not allowed".to_string(),
            request_bytes: 0,
            response_bytes: 0,
        });
    }
    Ok(())
}

fn reqwest_method(method: NetworkMethod) -> reqwest::Method {
    match method {
        NetworkMethod::Get => reqwest::Method::GET,
        NetworkMethod::Post => reqwest::Method::POST,
        NetworkMethod::Put => reqwest::Method::PUT,
        NetworkMethod::Patch => reqwest::Method::PATCH,
        NetworkMethod::Delete => reqwest::Method::DELETE,
        NetworkMethod::Head => reqwest::Method::HEAD,
    }
}

fn reqwest_error_diagnostic(error: &reqwest::Error) -> String {
    // Display/Debug include request URLs (including injected query secrets),
    // and nested backend messages may also include addresses or raw data.
    // Preserve useful failure classification without retaining those strings.
    let category = if error.is_timeout() {
        "request timed out"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_builder() {
        "request construction failed"
    } else if error.is_body() {
        "response body transfer failed"
    } else if error.is_decode() {
        "response decoding failed"
    } else if error.is_redirect() {
        "redirect failed"
    } else if error.is_request() {
        "request transfer failed"
    } else {
        "HTTP transport failed"
    };
    match error.status() {
        Some(status) => format!("{category} (HTTP {})", status.as_u16()),
        None => category.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::*;
    use reqwest::Method;

    // HTTPS port used in client-cache key tests (standard TLS port, RFC 8446).
    const TEST_HTTPS_PORT: u16 = 443;

    #[test]
    fn take_request_url_scrubs_only_source_carrier_copy() {
        let mut request = NetworkTransportRequest {
            method: NetworkMethod::Get,
            url: "https://api.example.test/v1?token=sk-query-secret".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            resolved_ips: Vec::new(),
            response_body_limit: None,
            timeout_ms: None,
        };

        let parsed = take_request_url(&mut request, 0).unwrap();

        assert_eq!(parsed.host_str(), Some("api.example.test"));
        assert_eq!(
            parsed.as_str(),
            "https://api.example.test/v1?token=sk-query-secret"
        );
        assert!(request.url.is_empty());
    }

    #[test]
    fn take_request_url_error_does_not_include_source_url() {
        let mut request = NetworkTransportRequest {
            method: NetworkMethod::Get,
            url: "https://api.example.test:bad-port/v1?token=sk-query-secret".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            resolved_ips: Vec::new(),
            response_body_limit: None,
            timeout_ms: None,
        };

        let error = take_request_url(&mut request, 0).unwrap_err();

        let NetworkHttpError::InvalidUrl { reason, .. } = error else {
            panic!("expected invalid URL error");
        };
        assert_eq!(reason, "URL parse error: invalid format");
        assert!(!reason.contains("api.example.test"));
        assert!(!reason.contains("sk-query-secret"));
        assert!(request.url.is_empty());
    }

    #[test]
    fn take_request_url_relative_error_does_not_include_source_url() {
        let mut request = NetworkTransportRequest {
            method: NetworkMethod::Get,
            url: "/relative/path?token=sk-query-secret".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            resolved_ips: Vec::new(),
            response_body_limit: None,
            timeout_ms: None,
        };

        let error = take_request_url(&mut request, 0).unwrap_err();

        let NetworkHttpError::InvalidUrl { reason, .. } = error else {
            panic!("expected invalid URL error");
        };
        assert_eq!(reason, "URL parse error: invalid format");
        assert!(!reason.contains("/relative/path"));
        assert!(!reason.contains("sk-query-secret"));
        assert!(request.url.is_empty());
    }

    #[test]
    fn apply_request_headers_zeroizes_source_carrier_copy_after_reqwest_build() {
        let client = reqwest::Client::new();
        let req = client.request(Method::GET, "http://example.com");
        let mut headers = vec![
            (
                "authorization".to_string(),
                "Bearer sk-header-secret".to_string(),
            ),
            (
                "x-api-key".to_string(),
                "sk-second-header-secret".to_string(),
            ),
        ];

        let req = apply_request_headers(req, &mut headers);

        assert_eq!(headers[0], (String::new(), String::new()));
        assert_eq!(headers[1], (String::new(), String::new()));
        let _ = req;
    }

    #[test]
    fn effective_request_timeout_retains_explicit_values_and_rejects_zero() {
        assert_eq!(
            effective_request_timeout(Some(60_000), Duration::from_secs(30), 0).unwrap(),
            Duration::from_secs(60)
        );
        assert_eq!(
            effective_request_timeout(Some(250), Duration::from_secs(30), 0).unwrap(),
            Duration::from_millis(250)
        );
        assert!(effective_request_timeout(Some(0), Duration::from_secs(30), 0).is_err());
        assert_eq!(
            effective_request_timeout(None, Duration::from_secs(30), 0).unwrap(),
            Duration::from_secs(30)
        );
    }

    #[tokio::test]
    async fn reqwest_transport_caches_clients_by_resolution_key() {
        let transport = ReqwestNetworkTransport::new(Duration::from_secs(1));
        let key = ReqwestClientKey {
            host: "api.example.test".to_string(),
            port: TEST_HTTPS_PORT,
            resolved_addrs: vec![SocketAddr::new(
                IpAddr::V4(std::net::Ipv4Addr::new(93, 184, 216, 34)),
                TEST_HTTPS_PORT,
            )],
            timeout: Duration::from_secs(1),
        };

        let _ = transport.client_for(key.clone(), 0).await.unwrap();
        let _ = transport.client_for(key, 0).await.unwrap();

        assert_eq!(transport.client_cache_status().unwrap().retained_entries, 1);
    }

    #[tokio::test]
    async fn cache_eviction_preserves_recent_and_active_clients_and_revision_fences() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let transport = ReqwestNetworkTransport::new(Duration::from_secs(2));
        let control = transport.client_cache_control();
        control.publish_capacity(0, 1, 2).unwrap();
        let key = |host: &str| ReqwestClientKey {
            host: host.into(),
            port: address.port(),
            resolved_addrs: vec![address],
            timeout: Duration::from_secs(2),
        };
        let first = key("one.example.test");
        let second = key("two.example.test");
        let third = key("three.example.test");
        let active = transport.client_for(first.clone(), 0).await.unwrap();
        transport.client_for(second.clone(), 0).await.unwrap();
        transport.client_for(first.clone(), 0).await.unwrap();
        transport.client_for(third.clone(), 0).await.unwrap();
        {
            let cache = transport.client_cache.lock().unwrap();
            assert_eq!(cache.entries.len(), 2);
            assert!(cache.entries.peek(&first).is_some());
            assert!(cache.entries.peek(&third).is_some());
            assert!(cache.entries.peek(&second).is_none());
        }
        control.publish_capacity(1, 2, 0).unwrap();
        assert_eq!(control.status().unwrap().retained_entries, 0);
        assert!(control.publish_capacity(1, 3, 2).is_err());
        assert!(control.publish_capacity(2, 2, 2).is_err());
        control.publish_capacity(1, 2, 0).unwrap(); // exact replay
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let received = stream.read(&mut request).unwrap();
            assert!(received > 0);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        });
        let response = active
            .get(format!("http://one.example.test:{}/test", address.port()))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(response.as_ref(), b"ok");
        server.join().unwrap();
        transport.client_for(first, 0).await.unwrap();
        assert_eq!(control.status().unwrap().retained_entries, 0);
    }

    #[test]
    fn client_build_admitted_before_publication_cannot_repopulate_successor_cache() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let (release, wait) = std::sync::mpsc::channel();
            let (started, ready) = tokio::sync::oneshot::channel();
            let occupied = tokio::task::spawn_blocking(move || {
                started.send(()).unwrap();
                wait.recv()
                    .expect("test releases the occupied builder thread");
            });
            ready.await.unwrap();
            let transport = ReqwestNetworkTransport::default();
            let mut build = Box::pin(transport.client_for(
                ReqwestClientKey {
                    host: "old.example.test".into(),
                    port: 443,
                    resolved_addrs: Vec::new(),
                    timeout: Duration::from_secs(2),
                },
                0,
            ));
            std::future::poll_fn(|context| {
                assert!(std::future::Future::poll(build.as_mut(), context).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            transport.publish_client_cache_capacity(0, 1, 4).unwrap();
            release.send(()).unwrap();
            build.await.unwrap();
            occupied.await.unwrap();
            assert_eq!(
                transport.client_cache_status().unwrap(),
                HttpClientCacheStatus {
                    revision: 1,
                    capacity: 4,
                    retained_entries: 0,
                    default_timeout: Duration::from_secs(30),
                    default_response_body_bytes: DEFAULT_RESPONSE_BODY_LIMIT,
                }
            );
        });
    }
}
