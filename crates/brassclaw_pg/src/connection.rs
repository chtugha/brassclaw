//! One parsed connection policy for pools and dedicated PostgreSQL sessions.
//! This layer opens connections; it never retries application SQL or effects.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio_postgres::config::{Host, SslMode};
use tokio_postgres::tls::{MakeTlsConnect, NoTlsStream};
use tokio_postgres::{AsyncMessage, Client, Config, Connection, NoTls, Socket};
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::PgError;

pub(crate) struct PreparedConnection {
    pub config: Config,
    pub tls: bool,
}

/// Parse the driver's URL/keyword grammar once, then inspect every endpoint.
/// Neither a query override nor `hostaddr` may bypass the transport policy.
pub(crate) fn prepare(connection_string: &str) -> Result<PreparedConnection, PgError> {
    let mut config: Config = connection_string
        .parse()
        .map_err(|_| PgError::Configuration("cannot parse connection string"))?;
    let hosts = config.get_hosts();
    let addresses = config.get_hostaddrs();
    let count = hosts.len().max(addresses.len());
    if count == 0 {
        // Unlike libpq, tokio-postgres has no implicit default socket directory.
        return Err(PgError::Configuration("host or hostaddr is required"));
    }
    if !hosts.is_empty() && !addresses.is_empty() && hosts.len() != addresses.len() {
        return Err(PgError::Configuration("host and hostaddr counts differ"));
    }
    if config.get_ports().len() > 1 && config.get_ports().len() != count {
        return Err(PgError::Configuration("port and endpoint counts differ"));
    }

    // Be conservative when a certificate hostname and hostaddr disagree.
    // Unspecified addresses are bind addresses, not proof of a local endpoint.
    let local = hosts.iter().all(|host| match host {
        #[cfg(unix)]
        Host::Unix(_) => true,
        Host::Tcp(name) => {
            name.eq_ignore_ascii_case("localhost")
                || name
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        }
    }) && addresses.iter().all(std::net::IpAddr::is_loopback);
    let tls = match config.get_ssl_mode() {
        SslMode::Disable if !local => return Err(PgError::RemoteClearTextDisabled),
        SslMode::Disable => false,
        SslMode::Prefer if local => false,
        SslMode::Prefer => {
            config.ssl_mode(SslMode::Require);
            true
        }
        SslMode::Require => true,
        _ => return Err(PgError::Configuration("unsupported TLS mode")),
    };
    if tls && config.get_hosts().is_empty() {
        // The driver requires a certificate hostname even when hostaddr selects
        // the socket address. An address-only DSN verifies that exact IP SAN.
        for address in config.get_hostaddrs().to_vec() {
            config.host(address.to_string());
        }
    }
    Ok(PreparedConnection { config, tls })
}

/// Certificate and hostname validation stays enabled, including explicit local
/// TLS. Native roots are preferred; the bundled Mozilla store is the fallback.
pub(crate) fn tls_connector() -> Result<MakeRustlsConnect, PgError> {
    let mut roots = rustls::RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for error in &native.errors {
        tracing::warn!("postgres: error loading system root certificates: {error}");
    }
    for certificate in native.certs {
        if let Err(error) = roots.add(certificate) {
            tracing::warn!("postgres: invalid system root certificate: {error}");
        }
    }
    if roots.is_empty() {
        tracing::info!("postgres: using bundled Mozilla certificate roots");
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    let config = rustls::ClientConfig::builder_with_provider(
        rustls::crypto::ring::default_provider().into(),
    )
    .with_safe_default_protocol_versions()
    .map_err(|_| PgError::Configuration("cannot configure TLS protocol versions"))?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(MakeRustlsConnect::new(config))
}

type RustlsStream = <MakeRustlsConnect as MakeTlsConnect<Socket>>::Stream;

enum Transport {
    Plain(Box<Connection<Socket, NoTlsStream>>),
    Tls(Box<Connection<Socket, RustlsStream>>),
}

/// Driver-owned dedicated connection, for session-local state such as LISTEN.
/// The caller must poll/spawn it and own its shutdown, as with tokio-postgres.
/// A dedicated session does not consume a pool checkout indefinitely.
pub struct PgConnection(Transport);

impl PgConnection {
    pub fn poll_message(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<AsyncMessage, tokio_postgres::Error>>> {
        match &mut self.0 {
            Transport::Plain(connection) => connection.poll_message(cx),
            Transport::Tls(connection) => connection.poll_message(cx),
        }
    }
}

impl Future for PgConnection {
    type Output = Result<(), tokio_postgres::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.get_mut().0 {
            Transport::Plain(connection) => Pin::new(connection.as_mut()).poll(cx),
            Transport::Tls(connection) => Pin::new(connection.as_mut()).poll(cx),
        }
    }
}

/// Open one dedicated session using the same policy as every shared pool.
pub async fn connect(connection_string: &str) -> Result<(Client, PgConnection), PgError> {
    let prepared = prepare(connection_string)?;
    if prepared.tls {
        let (client, connection) = prepared
            .config
            .connect(tls_connector()?)
            .await
            .map_err(|error| PgError::Connect(error.to_string()))?;
        Ok((client, PgConnection(Transport::Tls(Box::new(connection)))))
    } else {
        let (client, connection) = prepared
            .config
            .connect(NoTls)
            .await
            .map_err(|error| PgError::Connect(error.to_string()))?;
        Ok((client, PgConnection(Transport::Plain(Box::new(connection)))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_endpoint_forms_keep_plaintext_compatibility() {
        for dsn in [
            "postgres://user:pass@localhost/db",
            "postgres://user@127.0.0.1:5432/db",
            "postgres://[::1]/db",
            "host=localhost user=brassclaw",
            "host=127.0.0.2 user=brassclaw",
            "hostaddr=127.0.0.1 user=brassclaw",
            "hostaddr=::1 user=brassclaw",
            "host=localhost,127.0.0.2 user=brassclaw",
            "postgresql:///db?host=127.0.0.1&user=brassclaw",
        ] {
            assert!(!prepare(dsn).expect("local connection").tls, "{dsn}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn socket_paths_use_the_driver_grammar() {
        for dsn in [
            "host=/var/run/postgresql user=brassclaw",
            "host='/var/run/postgresql' user=brassclaw",
        ] {
            assert!(!prepare(dsn).expect("socket connection").tls);
        }
        assert!(
            prepare("host=/var/run/postgresql,db.example.com user=brassclaw")
                .expect("mixed endpoints")
                .tls
        );
    }

    #[test]
    fn every_remote_or_ambiguous_endpoint_requires_tls() {
        for dsn in [
            "postgres://user@db.internal/db",
            "postgres://user@10.0.0.5/db",
            "postgres://user@[2001:db8::1]/db",
            "hostaddr=10.0.0.5 user=brassclaw",
            "hostaddr=2001:db8::1 user=brassclaw",
            "host=localhost,db.internal user=brassclaw",
            "host=localhost hostaddr=10.0.0.5 user=brassclaw",
            "host=db.internal hostaddr=127.0.0.1 user=brassclaw",
            "host=0.0.0.0 user=brassclaw",
            "hostaddr=:: user=brassclaw",
            "postgresql:///db?host=db.example.com&user=brassclaw",
            "postgresql://localhost/db?host=db.example.com&user=brassclaw",
            "host = db.internal user=brassclaw",
            "host =db.internal user=brassclaw",
            "host= db.internal user=brassclaw",
        ] {
            let prepared = prepare(dsn).expect("remote connection");
            assert!(prepared.tls, "{dsn}");
            assert_eq!(prepared.config.get_ssl_mode(), SslMode::Require, "{dsn}");
            let separator = if dsn.contains("://") {
                if dsn.contains('?') { "&" } else { "?" }
            } else {
                " "
            };
            let disabled = format!("{dsn}{separator}sslmode=disable");
            assert!(matches!(
                prepare(&disabled),
                Err(PgError::RemoteClearTextDisabled)
            ));
        }
    }

    #[test]
    fn explicit_tls_is_preserved_on_loopback() {
        let prepared = prepare("postgres://user@localhost/db?sslmode=require").expect("local TLS");
        assert!(prepared.tls);
        assert_eq!(prepared.config.get_ssl_mode(), SslMode::Require);
        assert!(matches!(
            prepare("postgres://user@db.internal/db?sslmode=disable"),
            Err(PgError::RemoteClearTextDisabled)
        ));
    }

    #[test]
    fn malformed_configuration_errors_do_not_expose_values() {
        for dsn in [
            "user=brassclaw password=private-password",
            "host=localhost password=private-password sslmode=private-password",
            "host=localhost,localhost hostaddr=127.0.0.1 password=private-password",
            "host=localhost port=5432,5433 password=private-password",
        ] {
            let error = prepare(dsn).err().expect("invalid configuration");
            assert!(!format!("{error:?} {error}").contains("private-password"));
        }
    }
}
