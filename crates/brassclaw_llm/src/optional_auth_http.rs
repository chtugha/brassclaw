//! Preserve explicit provider auth; never transmit an invented SDK key.
use bytes::Bytes;
use rig::{http_client::sse::BoxedStream, http_client::*, wasm_compat::*};

/// Typed HTTP rejection retains only private context-parsing material. Display
/// and Debug never expose an upstream body, request URL or credential header.
pub(crate) struct ProviderHttpRejection {
    pub(crate) status: u16,
    pub(crate) retry_after: Option<std::time::Duration>,
    pub(crate) body: String,
}
impl std::fmt::Debug for ProviderHttpRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderHttpRejection")
            .field("status", &self.status)
            .field("retry_after", &self.retry_after)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for ProviderHttpRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "provider HTTP rejection {}", self.status)
    }
}
impl std::error::Error for ProviderHttpRejection {}

#[derive(Clone, Default)]
pub(crate) struct OptionalAuthHttp {
    pub(crate) client: reqwest::Client,
    // None keeps SDK key auth. Some(None) omits auth; Some(Some(_)) restores
    // an explicitly configured custom header when no SDK API key was supplied.
    pub(crate) auth_override: Option<Option<HeaderValue>>,
    /// Convert an SDK query key into the protocol's sensitive header before I/O.
    pub(crate) query_key_header: Option<&'static str>,
    pub(crate) max_response_bytes: Option<u64>,
}
impl std::fmt::Debug for OptionalAuthHttp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OptionalAuthHttp")
            .field("auth_override", &self.auth_override.is_some())
            .finish_non_exhaustive()
    }
}
impl OptionalAuthHttp {
    fn prepare<T>(&self, mut request: Request<T>) -> Result<Request<T>> {
        if let Some(auth) = &self.auth_override {
            request.headers_mut().remove(reqwest::header::AUTHORIZATION);
            if let Some(value) = auth {
                request
                    .headers_mut()
                    .insert(reqwest::header::AUTHORIZATION, value.clone());
            }
        }
        if let Some(header) = self.query_key_header {
            use secrecy::ExposeSecret;
            let invalid = || {
                Error::Instance(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid provider credential transport",
                )))
            };
            let mut url = reqwest::Url::parse(&request.uri().to_string()).map_err(|_| invalid())?;
            let mut credential = None;
            let mut retained = Vec::new();
            for (name, value) in url.query_pairs() {
                if name == "key" {
                    if credential.is_some() || value.is_empty() {
                        return Err(invalid());
                    }
                    credential = Some(secrecy::SecretString::from(value.into_owned()));
                } else {
                    retained.push((name.into_owned(), value.into_owned()));
                }
            }
            let credential = credential.ok_or_else(invalid)?;
            let mut value =
                HeaderValue::from_str(credential.expose_secret()).map_err(|_| invalid())?;
            value.set_sensitive(true);
            request.headers_mut().insert(header, value);
            url.set_query(None);
            if !retained.is_empty() {
                url.query_pairs_mut().extend_pairs(retained);
            }
            *request.uri_mut() = url.as_str().parse().map_err(|_| invalid())?;
        }
        if let Some(value) = request
            .headers_mut()
            .get_mut(reqwest::header::AUTHORIZATION)
        {
            value.set_sensitive(true);
        }
        Ok(request)
    }
}
fn body_error(error: crate::response_body::ResponseBodyError) -> Error {
    Error::Instance(Box::new(error))
}
async fn checked_response(
    response: reqwest::Response,
    limit: Option<u64>,
) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        crate::response_body::check_length(&response, limit).map_err(body_error)?;
        return Ok(response);
    }
    let retry_after =
        crate::retry::parse_retry_after_hint(response.headers().get(reqwest::header::RETRY_AFTER));
    // Only context rejection classification needs the body. Drop other error
    // bodies without buffering, forwarding or logging their private contents.
    let body = if matches!(status.as_u16(), 400 | 413) {
        let bytes = crate::response_body::read(response, limit)
            .await
            .map_err(body_error)?;
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        String::new()
    };
    Err(Error::Instance(Box::new(ProviderHttpRejection {
        status: status.as_u16(),
        retry_after,
        body,
    })))
}
fn response_headers(response: &reqwest::Response) -> Result<Response<()>> {
    let mut output = Response::builder()
        .status(response.status())
        .version(response.version());
    if let Some(headers) = output.headers_mut() {
        *headers = response.headers().clone();
    }
    output.body(()).map_err(Error::Protocol)
}
async fn buffered_response<U: From<Bytes> + WasmCompatSend + 'static>(
    response: reqwest::Response,
    limit: Option<u64>,
    observer: Option<std::sync::Arc<dyn crate::provider_observation::ProviderWireObserver>>,
) -> Result<Response<LazyBody<U>>> {
    if let Some(observer) = observer {
        let status = response.status();
        let retry_after = crate::retry::parse_retry_after_hint(
            response.headers().get(reqwest::header::RETRY_AFTER),
        );
        let (parts, _) = response_headers(&response)?.into_parts();
        let bytes = crate::response_body::read(response, limit)
            .await
            .map_err(body_error)?;
        observer
            .receive_response(status.as_u16(), &bytes)
            .map_err(|error| Error::Instance(Box::new(error)))?;
        observer
            .persist_response()
            .await
            .map_err(|error| Error::Instance(Box::new(error)))?;
        if !status.is_success() {
            let body = if matches!(status.as_u16(), 400 | 413) {
                String::from_utf8_lossy(&bytes).into_owned()
            } else {
                String::new()
            };
            return Err(Error::Instance(Box::new(ProviderHttpRejection {
                status: status.as_u16(),
                retry_after,
                body,
            })));
        }
        let body: LazyBody<U> = Box::pin(async move { Ok(U::from(bytes)) });
        return Ok(Response::from_parts(parts, body));
    }
    let response = checked_response(response, limit).await?;
    let (parts, _) = response_headers(&response)?.into_parts();
    let body: LazyBody<U> = Box::pin(async move {
        crate::response_body::read(response, limit)
            .await
            .map(U::from)
            .map_err(body_error)
    });
    Ok(Response::from_parts(parts, body))
}
impl HttpClientExt for OptionalAuthHttp {
    fn send<T, U>(
        &self,
        request: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        T: Into<Bytes> + WasmCompatSend,
        U: From<Bytes> + WasmCompatSend + 'static,
    {
        let observer = (request.method() == reqwest::Method::POST
            && request.uri().path().ends_with("/chat/completions"))
        .then(crate::provider_observation::current)
        .flatten();
        let prepared = self.prepare(request.map(Into::<Bytes>::into));
        let client = self.client.clone();
        let limit = self.max_response_bytes;
        async move {
            let (parts, body) = prepared?.into_parts();
            let response = client
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .body(body)
                .send()
                .await
                .map_err(|error| Error::Instance(Box::new(error)))?;
            buffered_response(response, limit, observer).await
        }
    }
    fn send_multipart<U>(
        &self,
        request: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        U: From<Bytes> + WasmCompatSend + 'static,
    {
        let prepared = self.prepare(request);
        let client = self.client.clone();
        let limit = self.max_response_bytes;
        async move {
            let (parts, body) = prepared?.into_parts();
            let response = client
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .multipart(reqwest::multipart::Form::from(body))
                .send()
                .await
                .map_err(|error| Error::Instance(Box::new(error)))?;
            buffered_response(response, limit, None).await
        }
    }
    fn send_streaming<T>(
        &self,
        request: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + WasmCompatSend
    where
        T: Into<Bytes>,
    {
        let prepared = self.prepare(request.map(Into::<Bytes>::into));
        let client = self.client.clone();
        let limit = self.max_response_bytes;
        async move {
            let (parts, body) = prepared?.into_parts();
            let response = client
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .body(body)
                .send()
                .await
                .map_err(|error| Error::Instance(Box::new(error)))?;
            let response = checked_response(response, limit).await?;
            let (parts, _) = response_headers(&response)?.into_parts();
            // try_unfold ends permanently on error. No oversized chunk is
            // delivered and the response/connection is dropped at termination.
            let stream = futures::stream::try_unfold(
                (response, 0u64),
                move |(mut response, consumed)| async move {
                    let Some(chunk) = response
                        .chunk()
                        .await
                        .map_err(|error| Error::Instance(Box::new(error)))?
                    else {
                        return Ok(None);
                    };
                    let consumed = consumed
                        .checked_add(chunk.len() as u64)
                        .filter(|bytes| limit.is_none_or(|limit| *bytes <= limit))
                        .ok_or_else(|| {
                            body_error(crate::response_body::ResponseBodyError::Capacity {
                                limit: limit.unwrap_or(u64::MAX),
                            })
                        })?;
                    Ok(Some((chunk, (response, consumed))))
                },
            );
            let body: BoxedStream = Box::pin(stream);
            Ok(Response::from_parts(parts, body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn actual_http_omits_missing_auth_and_preserves_configured_auth() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let reader = tokio::spawn(async move {
            let mut observations = Vec::new();
            for _ in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 1024];
                while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    assert!(bytes.len() < 16_384);
                }
                let headers = String::from_utf8(bytes).unwrap();
                observations.push(
                    headers
                        .lines()
                        .find_map(|line| line.strip_prefix("authorization: "))
                        .map(str::to_owned),
                );
                socket
                    .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                    .await
                    .unwrap();
            }
            observations
        });
        for auth_override in [
            Some(None),
            None,
            Some(Some(HeaderValue::from_static("Custom explicit-test-auth"))),
        ] {
            let client = OptionalAuthHttp {
                client: reqwest::Client::new(),
                auth_override,
                query_key_header: None,
                max_response_bytes: None,
            };
            let request = Request::builder()
                .method("POST")
                .uri(format!("http://{address}/probe"))
                .header("authorization", "Bearer configured-test-key")
                .body(Bytes::new())
                .unwrap();
            let response: Response<LazyBody<Bytes>> = client.send(request).await.unwrap();
            response.into_body().await.unwrap();
        }
        assert_eq!(
            reader.await.unwrap(),
            vec![
                None,
                Some("Bearer configured-test-key".into()),
                Some("Custom explicit-test-auth".into())
            ]
        );
    }
    #[tokio::test]
    async fn actual_response_capacity_rejects_declared_chunked_and_streaming_overflow() {
        use futures::StreamExt;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for response in [
                "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
                "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\npass",
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\nab\r\n3\r\ncde\r\n0\r\n\r\n",
                "HTTP/1.1 400 Bad Request\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nerror\r\n0\r\n\r\n",
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\nab\r\n3\r\ncde\r\n0\r\n\r\n",
                "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
                "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                loop {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                        let length = headers.lines().find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .map(|value| value.parse::<usize>().unwrap())
                        });
                        if length.is_some_and(|length| request.len() >= end + 4 + length)
                            || (length.is_none()
                                && (!headers.contains("transfer-encoding: chunked")
                                    || request[end + 4..].ends_with(b"0\r\n\r\n")))
                        {
                            break;
                        }
                    }
                }
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let mut client = OptionalAuthHttp {
            client: reqwest::Client::new(),
            max_response_bytes: Some(4),
            ..Default::default()
        };
        let request = || {
            Request::builder()
                .uri(format!("http://{address}/body"))
                .body(Bytes::new())
                .unwrap()
        };
        fn capacity(error: &Error) {
            assert!(
                matches!(error, Error::Instance(error) if matches!(error.downcast_ref::<crate::response_body::ResponseBodyError>(), Some(crate::response_body::ResponseBodyError::Capacity { limit: 4 })))
            );
        }
        capacity(
            &client
                .send::<_, Bytes>(request())
                .await
                .err()
                .expect("response capacity failure"),
        );
        let exact = client
            .send::<_, Bytes>(request())
            .await
            .unwrap()
            .into_body()
            .await
            .unwrap();
        assert_eq!(exact, Bytes::from_static(b"pass"));
        let response = client.send::<_, Bytes>(request()).await.unwrap();
        capacity(&response.into_body().await.unwrap_err());
        capacity(
            &client
                .send::<_, Bytes>(request())
                .await
                .err()
                .expect("response capacity failure"),
        );
        let mut stream = client.send_streaming(request()).await.unwrap().into_body();
        let mut accepted = Vec::new();
        let mut rejected = false;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => accepted.extend_from_slice(&chunk),
                Err(error) => {
                    capacity(&error);
                    rejected = true;
                }
            }
        }
        assert!(rejected && accepted.len() <= 4);
        assert!(
            stream.next().await.is_none(),
            "the exchange stays terminated after overflow"
        );
        client.max_response_bytes = None;
        assert_eq!(
            client
                .send::<_, Bytes>(request())
                .await
                .unwrap()
                .into_body()
                .await
                .unwrap(),
            Bytes::from_static(b"hello")
        );
        client.max_response_bytes = Some(4);
        let multipart = Request::builder()
            .method("POST")
            .uri(format!("http://{address}/multipart"))
            .body(MultipartForm::new())
            .unwrap();
        capacity(
            &client
                .send_multipart::<Bytes>(multipart)
                .await
                .err()
                .expect("multipart body capacity"),
        );
        server.await.unwrap();
    }
}
