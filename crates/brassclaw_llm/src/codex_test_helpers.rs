//! Shared test helpers for OpenAI Codex provider tests.

use crate::config::OpenAiCodexConfig;

/// Build a minimal JWT for testing (header.payload.signature).
pub(crate) fn make_test_jwt(account_id: &str) -> String {
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let header = engine.encode(b"{\"alg\":\"RS256\",\"typ\":\"JWT\"}");
    let payload_json = serde_json::json!({
        "sub": "user123",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": account_id,
        },
    });
    let payload = engine.encode(payload_json.to_string().as_bytes());
    let sig = engine.encode(b"fake-signature");
    format!("{header}.{payload}.{sig}")
}

/// Build a test `OpenAiCodexConfig` with a given session path.
pub(crate) fn test_codex_config(session_path: std::path::PathBuf) -> OpenAiCodexConfig {
    OpenAiCodexConfig {
        model: "gpt-5.3-codex".to_string(),
        auth_endpoint: "https://auth.openai.com".to_string(),
        api_base_url: "https://chatgpt.com/backend-api/codex".to_string(),
        client_id: "test_client_id".to_string(),
        session_path,
        token_refresh_margin_secs: 300,
    }
}

/// Real loopback OAuth/model peer; no production credential or network service.
pub(crate) struct CodexHttpFixture {
    pub(crate) endpoint: String,
    pub(crate) requests: std::sync::Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
    stop: tokio::sync::watch::Sender<bool>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl CodexHttpFixture {
    pub(crate) async fn finish(mut self) {
        self.stop.send(true).unwrap();
        self.task.take().unwrap().await.unwrap();
    }
}
impl Drop for CodexHttpFixture {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

pub(crate) async fn codex_http_fixture(
    reply: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static,
) -> CodexHttpFixture {
    codex_http_fixture_with_headers(move |path, body| {
        let (status, body) = reply(path, body);
        (status, String::new(), body)
    })
    .await
}

pub(crate) async fn codex_http_fixture_with_headers(
    reply: impl Fn(&str, &str) -> (u16, String, String) + Send + Sync + 'static,
) -> CodexHttpFixture {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests: std::sync::Arc<tokio::sync::Mutex<Vec<(String, String)>>> = Default::default();
    let recorded = requests.clone();
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = tokio::select! {
                _ = stopped.changed() => break,
                accepted = listener.accept() => accepted.unwrap(),
            };
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let (path, body) = loop {
                let read = socket.read(&mut buffer).await.unwrap();
                assert!(read > 0, "fixture request must complete");
                bytes.extend_from_slice(&buffer[..read]);
                assert!(
                    bytes.len() < 64 * 1024,
                    "fixture expects a small complete OAuth request"
                );
                if let Some(end) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        let path = headers
                            .lines()
                            .next()
                            .unwrap()
                            .split_whitespace()
                            .nth(1)
                            .unwrap()
                            .to_owned();
                        let body =
                            String::from_utf8(bytes[end + 4..end + 4 + length].to_vec()).unwrap();
                        break (path, body);
                    }
                }
            };
            let (status, response_headers, response_body) = reply(&path, &body);
            recorded.lock().await.push((path, body));
            socket.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{response_headers}\r\n{response_body}",response_body.len()).as_bytes()).await.unwrap();
        }
    });
    CodexHttpFixture {
        endpoint: format!("http://{address}"),
        requests,
        stop,
        task: Some(task),
    }
}
