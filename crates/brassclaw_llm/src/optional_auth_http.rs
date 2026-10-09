//! Preserve explicit provider auth; never transmit an invented SDK key.
use bytes::Bytes;
use rig::{http_client::*, wasm_compat::*};

#[derive(Clone, Default)]
pub(crate) struct OptionalAuthHttp {
    pub(crate) client: reqwest::Client,
    // None keeps SDK key auth. Some(None) omits auth; Some(Some(_)) restores
    // an explicitly configured custom header when no SDK API key was supplied.
    pub(crate) auth_override: Option<Option<HeaderValue>>,
}
impl std::fmt::Debug for OptionalAuthHttp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OptionalAuthHttp")
            .field("auth_override", &self.auth_override.is_some())
            .finish_non_exhaustive()
    }
}
impl OptionalAuthHttp {
    fn prepare<T>(&self, mut request: Request<T>) -> Request<T> {
        if let Some(auth) = &self.auth_override {
            request.headers_mut().remove(reqwest::header::AUTHORIZATION);
            if let Some(value) = auth {
                request
                    .headers_mut()
                    .insert(reqwest::header::AUTHORIZATION, value.clone());
            }
        }
        request
    }
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
        self.client.send(self.prepare(request))
    }
    fn send_multipart<U>(
        &self,
        request: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        U: From<Bytes> + WasmCompatSend + 'static,
    {
        self.client.send_multipart(self.prepare(request))
    }
    fn send_streaming<T>(
        &self,
        request: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + WasmCompatSend
    where
        T: Into<Bytes>,
    {
        self.client.send_streaming(self.prepare(request))
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
}
