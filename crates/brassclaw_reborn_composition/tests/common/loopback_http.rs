//! Real loopback HTTP lifecycle for native transport tests. This is test-only
//! support, not a production product/API listener or provider response stand-in.

pub(super) async fn spawn(
    router: axum::Router,
) -> (
    std::net::SocketAddr,
    tokio_util::sync::CancellationToken,
    tokio_util::task::AbortOnDropHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let stop = tokio_util::sync::CancellationToken::new();
    let stopped = stop.clone();
    let server = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(stopped.cancelled_owned())
            .await
            .unwrap();
    }));
    (address, stop, server)
}
