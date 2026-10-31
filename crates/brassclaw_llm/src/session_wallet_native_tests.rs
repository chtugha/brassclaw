//! Real loopback auth transport with controlled issuer data, not wallet identity.
use super::*;
use crate::codex_test_helpers::{codex_http_fixture, codex_http_fixture_with_headers};
use serde_json::json;

fn signed() -> NearWalletSignedMessage {
    NearWalletSignedMessage {
        account_id: "alice.near".into(),
        public_key: "ed25519:fixture".into(),
        signature: "fixture-private-signature".into(),
        message: NEAR_WALLET_LOGIN_MESSAGE.into(),
        recipient: NEAR_WALLET_LOGIN_RECIPIENT.into(),
        nonce: vec![7; 32],
        callback_url: None,
    }
}
fn configuration(endpoint: &str, directory: &tempfile::TempDir) -> SessionConfig {
    SessionConfig {
        auth_base_url: endpoint.into(),
        session_path: directory.path().join("session.json"),
    }
}
#[tokio::test]
async fn wallet_http_returns_credentials_without_changing_file_or_live_session() {
    use secrecy::ExposeSecret as _;
    let peer = codex_http_fixture(|path, body| {
        assert_eq!(path, "/v1/auth/near");
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            value["signed_message"]["signature"],
            "fixture-private-signature"
        );
        assert_eq!(value["payload"]["nonce"], json!(vec![7; 32]));
        (
            200,
            json!({"access_token":"fresh-private-token"}).to_string(),
        )
    })
    .await;
    let directory = tempfile::tempdir().unwrap();
    let config = configuration(&peer.endpoint, &directory);
    let manager = SessionManager::new(config.clone());
    manager
        .save_session_for_renewer("original-private-token", Some("test"))
        .await
        .unwrap();
    let file = std::fs::read(&config.session_path).unwrap();
    let token = manager
        .near_wallet_login_with_policy(&signed(), crate::ProviderRequestPolicy::default())
        .await
        .unwrap();
    assert_eq!(token, "fresh-private-token");
    assert_eq!(
        manager.get_token().await.unwrap().expose_secret(),
        "original-private-token"
    );
    assert_eq!(std::fs::read(&config.session_path).unwrap(), file);
    assert_eq!(peer.requests.lock().await.len(), 1);
    peer.finish().await;
}
#[tokio::test]
async fn wallet_status_parse_and_capacity_errors_never_expose_issuer_payloads() {
    for (status, body, limit) in [
        (
            500,
            json!({"error":"private-issuer-error","access_token":"private-issuer-token"})
                .to_string(),
            None,
        ),
        (
            200,
            json!({"access_token":{"value":"private-issuer-token"}}).to_string(),
            None,
        ),
        (
            200,
            json!({"access_token":"private-issuer-token"}).to_string(),
            Some(4),
        ),
    ] {
        let peer = codex_http_fixture(move |_, _| (status, body.clone())).await;
        let directory = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(configuration(&peer.endpoint, &directory));
        let error = manager
            .near_wallet_login_with_policy(
                &signed(),
                crate::ProviderRequestPolicy {
                    max_response_bytes: limit,
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        let text = error.to_string();
        assert!(!text.contains("private-issuer-token"));
        assert!(!text.contains("private-issuer-error"));
        assert_eq!(peer.requests.lock().await.len(), 1);
        assert!(!manager.has_token().await);
        peer.finish().await;
    }
}
#[tokio::test]
async fn wallet_redirect_cannot_forward_the_signed_request() {
    let foreign =
        codex_http_fixture(|_, _| (200, json!({"access_token":"must-not-return"}).to_string()))
            .await;
    let location = format!("Location: {}/foreign-login\r\n", foreign.endpoint);
    let issuer =
        codex_http_fixture_with_headers(move |_, _| (307, location.clone(), String::new())).await;
    let directory = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(configuration(&issuer.endpoint, &directory));
    assert!(
        manager
            .near_wallet_login(&signed())
            .await
            .unwrap_err()
            .to_string()
            .contains("307")
    );
    assert_eq!(issuer.requests.lock().await.len(), 1);
    assert!(foreign.requests.lock().await.is_empty());
    issuer.finish().await;
    foreign.finish().await;
}
#[tokio::test]
async fn session_private_values_stay_out_of_debug_parse_and_validation_errors() {
    let peer = codex_http_fixture(|path, _| {
        assert_eq!(path, "/v1/users/me");
        (
            500,
            json!({"error":"private-validation-response"}).to_string(),
        )
    })
    .await;
    let directory = tempfile::tempdir().unwrap();
    let config = configuration(&peer.endpoint, &directory);
    let mut manager = SessionManager::new(config.clone());
    manager
        .save_session_for_renewer("private-saved-token", None)
        .await
        .unwrap();
    assert!(
        !manager
            .validate_token()
            .await
            .unwrap_err()
            .to_string()
            .contains("private-validation-response")
    );
    let record = SessionData {
        session_token: "private-debug-token".into(),
        created_at: Utc::now(),
        auth_provider: None,
    };
    assert!(!format!("{record:?}").contains("private-debug-token"));
    assert!(!format!("{:?}", signed()).contains("fixture-private-signature"));
    std::fs::write(
        &config.session_path,
        r#"{"session_token":"private-saved-token","created_at":"private-invalid-date"}"#,
    )
    .unwrap();
    assert!(
        !manager
            .load_session()
            .await
            .unwrap_err()
            .to_string()
            .contains("private-invalid-date")
    );
    // A real client constructor error must remain an error, never a new client
    // that silently loses its deadline/redirect policy.
    manager.client = reqwest::Client::builder().user_agent("\n").build();
    assert!(manager.client.is_err());
    assert!(
        manager
            .validate_token()
            .await
            .unwrap_err()
            .to_string()
            .contains("could not be constructed")
    );
    assert_eq!(peer.requests.lock().await.len(), 1);
    peer.finish().await;
}
