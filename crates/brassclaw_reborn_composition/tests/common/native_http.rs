//! Loopback protocol fixtures shared by native provider acceptance tests.
//! Production ingress remains owned by its host; this module cannot ship.
#![cfg(test)]

use std::future::Future;

pub(crate) async fn bind_loopback() -> std::io::Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind("127.0.0.1:0").await
}

pub(crate) async fn serve<F>(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    shutdown: F,
) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
}

/// Optional real-browser acceptance against the native test's actual facade
/// and database. The configured script is test tooling, never a runtime port.
/// Ordinary ingress authentication has separate native acceptance coverage.
#[cfg(all(feature = "postgres", feature = "skills-db"))]
pub(crate) async fn tool_policy_browser_acceptance(router: axum::Router) {
    let Some(script) = std::env::var_os("BRASSCLAW_TOOL_POLICY_BROWSER_ACCEPTANCE") else {
        return;
    };
    let listener = bind_loopback().await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(serve(listener, router, async {
        let _ = stopped.await;
    }));
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("node")
            .arg(script)
            .arg(format!("http://{address}"))
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../brassclaw_webui_v2_static/static"
            ))
            .output()
    })
    .await
    .unwrap()
    .unwrap();
    let _ = shutdown.send(());
    server.await.unwrap().unwrap();
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
