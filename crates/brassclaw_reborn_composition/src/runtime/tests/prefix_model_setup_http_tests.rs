//! Real operator middleware -> product facade -> native append-only drafts.
use super::{Value, at, json};
use axum::{Router, http::StatusCode};

pub(super) async fn exercise(app: &Router, specification: &Value) {
    let path = format!(
        "/api/webchat/v2/prefix-model-specifications/{}/setup-draft",
        specification["id"].as_str().unwrap()
    );
    let request = json!({"operation_id":uuid::Uuid::new_v4(),"expected_revision":0,"edits":[{"field":"max_prefix_tokens","value":0},{"field":"reserved_output_tokens","value":null}]});
    for method in ["GET", "PUT"] {
        assert_eq!(
            at(
                app,
                method,
                &path,
                (method == "PUT").then_some(&request),
                false
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    let (status, basis) = at(app, "GET", &path, None, true).await;
    assert_eq!(status, StatusCode::OK, "{basis}");
    assert_eq!(basis["revision"], 0);
    assert!(basis["operation_id"].is_null());
    assert_eq!(
        at(app, "GET", &format!("{path}?revision=0"), None, true)
            .await
            .1,
        basis
    );
    for query in ["revision=-1", "revision=wrong", "unknown=1"] {
        assert_eq!(
            at(app, "GET", &format!("{path}?{query}"), None, true)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, first) = at(app, "PUT", &path, Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["revision"], 1);
    assert_eq!(first["operation_id"], request["operation_id"]);
    assert_eq!(
        first["fields"]["max_prefix_tokens"],
        json!({"value":0,"origin":"manual"})
    );
    assert_eq!(
        first["fields"]["reserved_output_tokens"],
        json!({"value":null,"origin":"manual"})
    );
    assert_eq!(
        first["fields"]["max_prompt_tokens"],
        basis["fields"]["max_prompt_tokens"]
    );
    let second_request = json!({"operation_id":uuid::Uuid::new_v4(),"expected_revision":1,"edits":[{"field":"max_prompt_tokens","value":777}]});
    let (status, second) = at(app, "PUT", &path, Some(&second_request), true).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["revision"], 2);
    assert_eq!(
        at(app, "PUT", &path, Some(&request), true).await.1,
        first,
        "original retry returns historical receipt"
    );
    assert_eq!(
        at(app, "GET", &path, None, true).await.1,
        second,
        "current head never rolls back on retry"
    );
    assert_eq!(
        at(app, "GET", &format!("{path}?revision=1"), None, true)
            .await
            .1,
        first
    );
    assert_eq!(
        at(app, "GET", &format!("{path}?revision=0"), None, true)
            .await
            .1,
        basis
    );
    let mut stale = request.clone();
    stale["operation_id"] = json!(uuid::Uuid::new_v4());
    assert_eq!(
        at(app, "PUT", &path, Some(&stale), true).await.0,
        StatusCode::CONFLICT
    );
    let mut changed = request.clone();
    changed["edits"][0]["value"] = json!(5);
    assert_eq!(
        at(app, "PUT", &path, Some(&changed), true).await.0,
        StatusCode::CONFLICT
    );
    for edit in [
        json!({"field":"max_prompt_tokens","value":0}),
        json!({"field":"max_prefix_tokens"}),
        json!({"field":"max_prefix_tokens","value":false}),
        json!({"field":"max_prefix_tokens","value":1,"origin":"provider_configuration"}),
    ] {
        let invalid =
            json!({"operation_id":uuid::Uuid::new_v4(),"expected_revision":2,"edits":[edit]});
        assert!(
            at(app, "PUT", &path, Some(&invalid), true)
                .await
                .0
                .is_client_error()
        );
    }
    assert_eq!(at(app, "GET", &path, None, true).await.1, second);
    if let Some(node) = std::env::var_os("BRASSCLAW_EXACT_SETUP_BROWSER_NODE") {
        let listener = super::super::native_http::bind_loopback().await.unwrap();
        let address = listener.local_addr().unwrap();
        let stop = tokio_util::sync::CancellationToken::new();
        let server_stop = stop.clone();
        let router = app.clone();
        let server = tokio::spawn(super::super::native_http::serve(
            listener,
            router,
            server_stop.cancelled_owned(),
        ));
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/browser/model-setup-draft.mjs");
        let output = tokio::process::Command::new(node)
            .arg(script)
            .arg(format!("http://{address}"))
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../brassclaw_webui_v2_static/static"),
            )
            .arg(specification.to_string())
            .output()
            .await
            .unwrap();
        stop.cancel();
        server.await.unwrap().unwrap();
        assert!(
            output.status.success(),
            "browser failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("native-model-setup-draft-browser: passed"),
            "{stdout}"
        );
        println!("{stdout}");
    }
}
