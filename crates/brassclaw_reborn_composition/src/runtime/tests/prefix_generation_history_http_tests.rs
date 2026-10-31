//! Actual retained-generation API and React caller; only responses/file reads
//! are delayed. Existing quarantine and immutable storage semantics stay real.
pub(super) async fn exercise(app: &axum::Router, generation: Option<&std::path::Path>) {
    let Some(node) = std::env::var_os("BRASSCLAW_PREFIX_HISTORY_BROWSER_NODE") else {
        return;
    };
    let listener = super::super::native_http::bind_loopback().await.unwrap();
    let address = listener.local_addr().unwrap();
    let stop = tokio_util::sync::CancellationToken::new();
    let router = app.clone();
    let server = tokio::spawn(super::super::native_http::serve(
        listener,
        router,
        stop.clone().cancelled_owned(),
    ));
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = tokio::process::Command::new(node)
        .arg(manifest.join("../../tests/browser/prefix-generation-history.mjs"))
        .arg(format!("http://{address}"))
        .arg(manifest.join("../brassclaw_webui_v2_static/static"))
        .arg(generation.map_or_else(|| "--designs".into(), std::path::Path::to_path_buf))
        .kill_on_drop(true)
        .output()
        .await;
    stop.cancel();
    server.await.unwrap().unwrap();
    let output = output.unwrap();
    assert!(
        output.status.success(),
        "browser failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let marker = if generation.is_some() {
        "native-prefix-history-browser: passed"
    } else {
        "native-prefix-design-view-browser: passed"
    };
    assert!(stdout.contains(marker), "{stdout}");
    println!("{stdout}");
}
