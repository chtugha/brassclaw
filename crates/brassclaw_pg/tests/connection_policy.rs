//! Actual server acceptance; no fake protocol endpoint or permissive verifier.
#![cfg(feature = "connection-integration")]

use std::time::Duration;

use brassclaw_pg::{PgError, connection::connect, pool::build_pool_with_max_size};
use tokio::time::timeout;

fn dsn(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("{name} must identify the real acceptance server"))
}

#[tokio::test]
async fn shared_pool_and_dedicated_session_deliver_real_notifications() {
    let dsn = dsn("TEST_PG_URL");
    let pool = build_pool_with_max_size(&dsn, 4).expect("local pool");
    assert_eq!(pool.status().max_size, 4);
    let (listener, mut connection) = connect(&dsn).await.expect("dedicated session");
    let (notifications, mut received) = tokio::sync::mpsc::channel(1);
    let driver = tokio::spawn(async move {
        use std::task::Poll;
        std::future::poll_fn(|cx| {
            loop {
                match connection.poll_message(cx) {
                    Poll::Ready(Some(Ok(tokio_postgres::AsyncMessage::Notification(message)))) => {
                        notifications
                            .try_send(message)
                            .expect("notification channel");
                    }
                    Poll::Ready(Some(Ok(_))) => {}
                    Poll::Ready(Some(Err(error))) => panic!("listener failed: {error}"),
                    Poll::Ready(None) => return Poll::Ready(()),
                    Poll::Pending => return Poll::Pending,
                }
            }
        })
        .await
    });
    listener
        .batch_execute("LISTEN brassclaw_connection_acceptance")
        .await
        .expect("LISTEN");
    let client = pool.get().await.expect("pool checkout");
    client
        .execute(
            "SELECT pg_notify('brassclaw_connection_acceptance', $1)",
            &[&"exact-payload"],
        )
        .await
        .expect("actual NOTIFY");
    let notification = timeout(Duration::from_secs(5), received.recv())
        .await
        .expect("notification deadline")
        .expect("notification");
    assert_eq!(notification.channel(), "brassclaw_connection_acceptance");
    assert_eq!(notification.payload(), "exact-payload");
    drop(listener);
    timeout(Duration::from_secs(5), driver)
        .await
        .expect("dedicated close")
        .expect("driver joined");
    drop(client);
    pool.close();
}

#[tokio::test]
async fn remote_plaintext_is_rejected_and_prefer_cannot_downgrade() {
    let remote = dsn("TEST_PG_REMOTE_URL");
    // The real server deliberately has ssl=off. Its remote-classified hostname
    // uses a loopback hostaddr so this acceptance never contacts a public host.
    let disabled = format!("{remote} sslmode=disable");
    assert!(matches!(
        build_pool_with_max_size(&disabled, 4),
        Err(PgError::RemoteClearTextDisabled)
    ));
    assert!(matches!(
        connect(&disabled).await,
        Err(PgError::RemoteClearTextDisabled)
    ));
    let pool = build_pool_with_max_size(&remote, 4).expect("remote prefer pool");
    assert!(
        pool.get().await.is_err(),
        "TLS refusal must not become plaintext success"
    );
    assert!(
        connect(&remote).await.is_err(),
        "dedicated session must not downgrade either"
    );
    pool.close();
}

#[tokio::test]
async fn local_and_remote_tls_validate_certificates_and_server_transport() {
    let tls = dsn("TEST_PG_TLS_URL");
    assert!(
        tls.parse::<tokio_postgres::Config>()
            .expect("TLS fixture")
            .get_hosts()
            .is_empty(),
        "TLS fixture must specify hostaddr only so each test owns its certificate hostname"
    );
    for host in ["localhost", "pg.brassclaw.test", "127.0.0.1"] {
        let connection_string = format!("{tls} host={host} sslmode=require");
        let pool = build_pool_with_max_size(&connection_string, 4).expect("TLS pool");
        let client = pool.get().await.expect("verified TLS checkout");
        let encrypted: bool = client
            .query_one(
                "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
                &[],
            )
            .await
            .expect("server transport proof")
            .get(0);
        assert!(encrypted);
        drop(client);
        pool.close();
        let (client, connection) = connect(&connection_string)
            .await
            .expect("verified dedicated TLS");
        let driver = tokio::spawn(connection);
        let encrypted: bool = client
            .query_one(
                "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
                &[],
            )
            .await
            .expect("dedicated transport proof")
            .get(0);
        assert!(encrypted);
        drop(client);
        timeout(Duration::from_secs(5), driver)
            .await
            .expect("TLS close")
            .expect("TLS driver joined")
            .expect("TLS driver closed cleanly");
    }
    // Same trusted certificate and server, wrong certificate identity.
    let wrong_hostname = format!("{tls} host=wrong.brassclaw.test sslmode=require");
    let pool = build_pool_with_max_size(&wrong_hostname, 4).expect("wrong hostname pool");
    assert!(
        pool.get().await.is_err(),
        "certificate identity must be verified"
    );
    assert!(connect(&wrong_hostname).await.is_err());
    pool.close();

    // Address-only TLS verifies the IP SAN rather than losing the hostname.
    let address_only = format!("{tls} sslmode=require");
    let (client, connection) = connect(&address_only).await.expect("IP SAN verified");
    let driver = tokio::spawn(connection);
    client
        .simple_query("SELECT 1")
        .await
        .expect("address-only query");
    drop(client);
    timeout(Duration::from_secs(5), driver)
        .await
        .expect("address-only close")
        .expect("address-only driver joined")
        .expect("address-only driver closed");

    let remote_prefer = format!("{tls} host=pg.brassclaw.test sslmode=prefer");
    let pool = build_pool_with_max_size(&remote_prefer, 4).expect("remote prefer TLS pool");
    let client = pool.get().await.expect("remote prefer verified checkout");
    let encrypted: bool = client
        .query_one(
            "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
            &[],
        )
        .await
        .expect("remote prefer transport proof")
        .get(0);
    assert!(encrypted);
    drop(client);
    pool.close();

    // A second real TLS server has a different issuer, absent from the trust
    // store. A TLS connector must not equate encryption with authentication.
    let untrusted = dsn("TEST_PG_UNTRUSTED_URL");
    let pool = build_pool_with_max_size(&untrusted, 4).expect("untrusted issuer pool");
    assert!(pool.get().await.is_err(), "issuer must be trusted");
    assert!(connect(&untrusted).await.is_err());
    pool.close();
}
