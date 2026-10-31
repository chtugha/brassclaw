//! Actual SDK HTTP and local persistence boundaries; no live OAuth account.
use super::*;
use crate::codex_test_helpers::{codex_http_fixture, test_codex_config};
use serde_json::{Value, json};

fn old_session() -> OpenAiCodexSession {
    OpenAiCodexSession {
        access_token: "existing-access".into(),
        refresh_token: "existing-refresh".into(),
        expires_at: Utc::now() + chrono::Duration::hours(1),
        created_at: Utc::now() - chrono::Duration::days(1),
    }
}

#[tokio::test]
async fn native_device_authorization_returns_credentials_without_publishing_session() {
    let server = codex_http_fixture(|path, body| match path {
        "/api/accounts/deviceauth/token" => {
            let body:Value=serde_json::from_str(body).unwrap();
            assert_eq!(body["device_auth_id"],"original-device");
            assert_eq!(body["user_code"],"operator-code");
            (200,json!({"authorization_code":"authorized-code","code_challenge":"opaque-challenge","code_verifier":"opaque-verifier"}).to_string())
        }
        "/oauth/token" => {
            assert!(body.contains("grant_type=authorization_code"));
            assert!(body.contains("code=authorized-code"));
            assert!(body.contains("code_verifier=opaque-verifier"));
            (200,json!({"access_token":"new-authorized-access","refresh_token":"new-authorized-refresh","expires_in":3600,"token_type":"Bearer"}).to_string())
        }
        other => panic!("unexpected OAuth path {other}"),
    }).await;
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("session.json");
    let mut configuration = test_codex_config(path.clone());
    configuration.auth_endpoint = server.endpoint.clone();
    let manager = OpenAiCodexSessionManager::new(configuration).unwrap();
    let old = old_session();
    manager.save_session(&old).await.unwrap();
    manager.set_session(old).await;
    let original = std::fs::read(&path).unwrap();
    let start = DeviceCodeStart {
        device_auth_id: "original-device".into(),
        user_code: "operator-code".into(),
        verification_uri: format!("{}/device", server.endpoint),
        interval_secs: 5,
        expires_secs: 60,
    };
    let returned = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        manager.authorize_device_code(&start),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(returned.access_token, "new-authorized-access");
    assert_eq!(returned.refresh_token, "new-authorized-refresh");
    assert_eq!(
        manager.session.read().await.as_ref().unwrap().access_token,
        "existing-access"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "host must commit role/credential before publication"
    );
    assert_eq!(server.requests.lock().await.len(), 2);
    server.finish().await;
}

#[tokio::test]
async fn native_refresh_keeps_unrotated_refresh_token_and_original_creation() {
    let server = codex_http_fixture(|path, body| {
        assert_eq!(path, "/oauth/token");
        assert!(body.contains("grant_type=refresh_token"));
        assert!(body.contains("refresh_token=existing-refresh"));
        (
            200,
            json!({"access_token":"refreshed-access","expires_in":3600,"token_type":"Bearer"})
                .to_string(),
        )
    })
    .await;
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("session.json");
    let mut configuration = test_codex_config(path.clone());
    configuration.auth_endpoint = server.endpoint.clone();
    let manager = OpenAiCodexSessionManager::new(configuration).unwrap();
    let mut old = old_session();
    old.expires_at = Utc::now() - chrono::Duration::seconds(1);
    let created = old.created_at;
    manager.save_session(&old).await.unwrap();
    manager.set_session(old).await;
    manager.refresh_tokens().await.unwrap();
    let state = manager.session.read().await;
    let updated = state.as_ref().unwrap();
    assert_eq!(updated.access_token, "refreshed-access");
    assert_eq!(updated.refresh_token, "existing-refresh");
    assert_eq!(updated.created_at, created);
    let saved: OpenAiCodexSession = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved.refresh_token, "existing-refresh");
    assert_eq!(saved.created_at, created);
    drop(state);
    assert_eq!(server.requests.lock().await.len(), 1);
    server.finish().await;
}

#[tokio::test]
async fn native_bad_expiry_or_response_cannot_replace_session_or_expose_payload() {
    for body in [
        json!({"access_token":"sensitive-issued-token","expires_in":u64::MAX,"token_type":"Bearer"}),
        json!({"access_token":"sensitive-issued-token","expires_in":i64::MAX as u64,"token_type":"Bearer"}),
        json!({"access_token":"sensitive-issued-token","expires_in":"sensitive-invalid-expiry","token_type":"Bearer"}),
        json!({"access_token":"sensitive-issued-token","expires_in":3600,"token_type":"unsupported"}),
    ] {
        let server = codex_http_fixture(move |path, _| {
            assert_eq!(path, "/oauth/token");
            (200, body.to_string())
        })
        .await;
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("session.json");
        let mut configuration = test_codex_config(path.clone());
        configuration.auth_endpoint = server.endpoint.clone();
        let manager = OpenAiCodexSessionManager::new(configuration).unwrap();
        let mut old = old_session();
        old.expires_at = Utc::now() - chrono::Duration::seconds(1);
        manager.save_session(&old).await.unwrap();
        manager.set_session(old).await;
        let original = std::fs::read(&path).unwrap();
        let result = manager.refresh_tokens().await;
        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(!message.contains("sensitive-issued-token"));
        assert!(!message.contains("sensitive-invalid-expiry"));
        assert_eq!(
            manager.session.read().await.as_ref().unwrap().access_token,
            "existing-access"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(server.requests.lock().await.len(), 1);
        server.finish().await;
    }
}

#[tokio::test]
async fn native_device_response_errors_are_private_and_expired_flow_never_polls() {
    for (status,body) in [
        (200,json!({"device_auth_id":"opaque-device","user_code":"operator-code","expires_in":"private-expiry-response"}).to_string()),
        (500,json!({"error":"private-expiry-response"}).to_string()),
        (200,json!({"device_auth_id":"opaque-device","user_code":"operator-code","expires_at":"private-expiry-response"}).to_string()),
    ] {
        let server=codex_http_fixture(move |path,_| {assert_eq!(path,"/api/accounts/deviceauth/usercode");(status,body.clone())}).await;
        let home=tempfile::tempdir().unwrap();
        let mut configuration=test_codex_config(home.path().join("session.json"));configuration.auth_endpoint=server.endpoint.clone();
        let manager=OpenAiCodexSessionManager::new(configuration).unwrap();
        let error=manager.initiate_device_code().await.unwrap_err().to_string();
        assert!(!error.contains("private-expiry-response"));
        assert!(!manager.has_session().await);assert_eq!(server.requests.lock().await.len(),1);
        let expired=DeviceCodeStart {device_auth_id:"expired".into(),user_code:"expired".into(),verification_uri:server.endpoint.clone(),interval_secs:u64::MAX,expires_secs:0};
        assert!(tokio::time::timeout(std::time::Duration::from_secs(1),manager.authorize_device_code(&expired)).await.unwrap().is_err());
        assert_eq!(server.requests.lock().await.len(),1,"expired flow cannot issue another HTTP request");
        server.finish().await;
    }
}

#[tokio::test]
async fn access_only_token_inside_refresh_margin_remains_usable_and_large_margin_is_safe() {
    let home = tempfile::tempdir().unwrap();
    let mut configuration = test_codex_config(home.path().join("session.json"));
    configuration.token_refresh_margin_secs = u64::MAX;
    let manager = OpenAiCodexSessionManager::new(configuration).unwrap();
    let mut old = old_session();
    old.refresh_token.clear();
    manager.set_session(old).await;
    assert!(
        manager.needs_refresh().await,
        "large configured margin cannot wrap into a negative value"
    );
    assert!(
        manager.get_access_token().await.is_ok(),
        "a still-valid token does not require an impossible refresh"
    );
}

#[tokio::test]
async fn native_refresh_never_forwards_credentials_to_redirect_target() {
    use crate::codex_test_helpers::codex_http_fixture_with_headers;
    let target = codex_http_fixture(|_, _| {
        (
            200,
            json!({"access_token":"forwarded-token","expires_in":3600}).to_string(),
        )
    })
    .await;
    let location = format!("Location: {}/foreign-token\r\n", target.endpoint);
    let issuer = codex_http_fixture_with_headers(move |path, _| {
        assert_eq!(path, "/oauth/token");
        (307, location.clone(), String::new())
    })
    .await;
    let home = tempfile::tempdir().unwrap();
    let mut configuration = test_codex_config(home.path().join("session.json"));
    configuration.auth_endpoint = issuer.endpoint.clone();
    let manager = OpenAiCodexSessionManager::new(configuration).unwrap();
    let mut old = old_session();
    old.expires_at = Utc::now() - chrono::Duration::seconds(1);
    manager.set_session(old).await;
    assert!(
        manager
            .refresh_tokens()
            .await
            .unwrap_err()
            .to_string()
            .contains("307")
    );
    assert_eq!(issuer.requests.lock().await.len(), 1);
    assert!(
        target.requests.lock().await.is_empty(),
        "OAuth redirect cannot carry a refresh token to another host"
    );
    issuer.finish().await;
    target.finish().await;
}
