//! Actual loopback PostgreSQL wire transport for native caller tests. It forwards
//! real bytes and can withhold a selected actual acknowledgement. It never
//! executes SQL, manufactures a result, interprets a task or logs credentials.
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering},
    },
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{oneshot, watch},
};
use tokio_util::sync::CancellationToken;

use crate::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckBoundary {
    Begin,
    Commit,
    IdleReady,
}

/// Caller-owned test selectors, not application admission or success evidence.
pub struct AckCutRule {
    id: u8,
    bind: Option<BindSelector>,
    boundary: AckBoundary,
    hold: bool,
    following_query: Option<Vec<u8>>,
}
struct BindSelector {
    required: Vec<Vec<u8>>,
    forbidden: Vec<Vec<u8>>,
}
impl AckCutRule {
    /// The private test pool must issue only the intended transaction while
    /// armed. Authentication and ordinary frames are still forwarded unchanged.
    pub fn next_commit(id: u8) -> Self {
        Self {
            id,
            bind: None,
            boundary: AckBoundary::Commit,
            hold: false,
            following_query: None,
        }
    }
    pub fn after_bind(
        id: u8,
        required: Vec<Vec<u8>>,
        forbidden: Vec<Vec<u8>>,
        boundary: AckBoundary,
    ) -> Self {
        Self {
            id,
            bind: Some(BindSelector {
                required,
                forbidden,
            }),
            boundary,
            hold: false,
            following_query: None,
        }
    }
    /// Hold the actual selected ACK until this arm's ticket releases it.
    pub fn hold(mut self) -> Self {
        self.hold = true;
        self
    }
    /// An additional exact frontend Query frame required before a BEGIN ACK.
    /// Bytes are caller-owned selectors, never SQL executed by this transport.
    pub fn following_query(mut self, query: Vec<u8>) -> Self {
        self.following_query = Some(query);
        self
    }
    fn valid(&self) -> bool {
        self.id != 0
            && (self.boundary != AckBoundary::Begin
                || (self.hold && self.bind.is_some() && self.following_query.is_some()))
            && self.following_query.as_ref().is_none_or(|query| {
                self.boundary == AckBoundary::Begin && query.len() > 1 && query.last() == Some(&0)
            })
            && self.bind.as_ref().is_none_or(|selector| {
                !selector.required.is_empty()
                    && selector
                        .required
                        .iter()
                        .chain(&selector.forbidden)
                        .all(|needle| !needle.is_empty())
            })
    }
    fn matches(&self, payload: &[u8]) -> bool {
        self.bind.as_ref().is_some_and(|selector| {
            let contains =
                |needle: &[u8]| payload.windows(needle.len()).any(|window| window == needle);
            selector.required.iter().map(Vec::as_slice).all(contains)
                && !selector.forbidden.iter().map(Vec::as_slice).any(contains)
        })
    }
}

/// Real protocol facts only; PID is public, the BackendKeyData cancel key is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckHoldObservation {
    pub generation: u64,
    pub session: usize,
    pub backend_pid: i32,
    pub boundary: AckBoundary,
    pub following_query_observed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckHoldEnd {
    PeerClosed,
    IoFailure,
    TicketDropped,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckHoldStatus {
    Armed,
    Held(AckHoldObservation),
    Released(AckHoldObservation),
    Closed(AckHoldEnd),
}

struct HoldControl {
    backend_pid: i32,
    status: watch::Sender<AckHoldStatus>,
    release: Mutex<Option<oneshot::Receiver<()>>>,
    abandoned: CancellationToken,
}
#[derive(Clone)]
struct Armed {
    id: u8,
    generation: u64,
    hold: Option<Arc<HoldControl>>,
}
#[derive(Default)]
struct ArmState {
    generation: u64,
    active: Option<Armed>,
}
#[derive(Default)]
struct TransportControl(Mutex<ArmState>);
impl TransportControl {
    fn selected(&self) -> io::Result<Option<Armed>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| io::Error::other("wire control poisoned"))?
            .active
            .clone())
    }
    fn arm(&self, id: u8, hold: Option<Arc<HoldControl>>) -> io::Result<u64> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| io::Error::other("wire control poisoned"))?;
        if state.active.is_some() {
            return Err(io::Error::other("ACK already armed"));
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| io::Error::other("ACK generation exhausted"))?;
        let generation = state.generation;
        state.active = Some(Armed {
            id,
            generation,
            hold,
        });
        Ok(generation)
    }
    fn clear(&self, generation: u64, status: Option<AckHoldStatus>) -> io::Result<bool> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| io::Error::other("wire control poisoned"))?;
        if state
            .active
            .as_ref()
            .is_none_or(|arm| arm.generation != generation)
        {
            return Ok(false);
        }
        if let Some(arm) = state.active.take()
            && let (Some(hold), Some(status)) = (arm.hold, status)
        {
            hold.status.send_replace(status);
        }
        Ok(true)
    }
    fn held(&self, observation: AckHoldObservation) -> io::Result<bool> {
        let state = self
            .0
            .lock()
            .map_err(|_| io::Error::other("wire control poisoned"))?;
        let Some(hold) = state
            .active
            .as_ref()
            .filter(|arm| arm.generation == observation.generation)
            .and_then(|arm| arm.hold.as_ref())
        else {
            return Ok(false);
        };
        hold.status.send_replace(AckHoldStatus::Held(observation));
        Ok(true)
    }
    fn close_session(&self, pid: i32, reason: AckHoldEnd) -> io::Result<()> {
        if let Some(arm) = self.selected()?
            && arm
                .hold
                .as_ref()
                .is_some_and(|hold| hold.backend_pid == pid)
        {
            self.clear(arm.generation, Some(AckHoldStatus::Closed(reason)))?;
        }
        Ok(())
    }
    fn close(&self, reason: AckHoldEnd) -> io::Result<()> {
        if let Some(arm) = self.selected()? {
            self.clear(arm.generation, Some(AckHoldStatus::Closed(reason)))?;
        }
        Ok(())
    }
}

/// One arm's non-clonable release capability. Status survives missed wakeups.
/// Dropping an unreleased ticket aborts its hold, never fabricates an ACK.
pub struct AckHoldTicket {
    generation: u64,
    control: Arc<TransportControl>,
    hold: Arc<HoldControl>,
    status: watch::Receiver<AckHoldStatus>,
    release: Option<oneshot::Sender<()>>,
}
impl AckHoldTicket {
    pub fn status(&self) -> AckHoldStatus {
        *self.status.borrow()
    }
    pub async fn wait_held(&mut self) -> io::Result<AckHoldObservation> {
        loop {
            let status = *self.status.borrow_and_update();
            match status {
                AckHoldStatus::Held(observation) => return Ok(observation),
                AckHoldStatus::Armed => {}
                _ => return Err(io::Error::other("hold ended before observation")),
            }
            self.status
                .changed()
                .await
                .map_err(|_| io::Error::other("hold observer closed"))?;
        }
    }
    pub async fn release(&mut self) -> io::Result<AckHoldObservation> {
        if !matches!(self.status(), AckHoldStatus::Held(_)) {
            return Err(io::Error::other("ACK is not held"));
        }
        self.release
            .take()
            .ok_or_else(|| io::Error::other("ACK release already consumed"))?
            .send(())
            .map_err(|_| io::Error::other("ACK hold ended"))?;
        loop {
            let status = *self.status.borrow_and_update();
            match status {
                AckHoldStatus::Released(observation) => return Ok(observation),
                AckHoldStatus::Closed(_) => return Err(io::Error::other("ACK release failed")),
                _ => {}
            }
            self.status
                .changed()
                .await
                .map_err(|_| io::Error::other("hold observer closed"))?;
        }
    }
    pub async fn wait_closed(&mut self) -> io::Result<AckHoldEnd> {
        loop {
            let status = *self.status.borrow_and_update();
            match status {
                AckHoldStatus::Closed(reason) => return Ok(reason),
                AckHoldStatus::Released(_) => return Err(io::Error::other("ACK was released")),
                _ => {}
            }
            self.status
                .changed()
                .await
                .map_err(|_| io::Error::other("hold observer closed"))?;
        }
    }
}
impl Drop for AckHoldTicket {
    fn drop(&mut self) {
        self.hold.abandoned.cancel();
        // A completed release has already removed its generation. A stale
        // ticket cannot clear or release a later arm, including the same rule ID.
        let _ = self.control.clear(
            self.generation,
            Some(AckHoldStatus::Closed(AckHoldEnd::TicketDropped)),
        );
    }
}

/// Owns the listener, forwarding connections and explicit shutdown. Product test
/// callers supply byte selectors and observe real database/coordinator results.
/// The connection string remains private and has no Debug implementation.
pub struct AckCutTransport {
    pub pool: Arc<PgPool>,
    connection: String,
    rules: Arc<BTreeMap<u8, AckCutRule>>,
    control: Arc<TransportControl>,
    cut: Arc<AtomicUsize>,
    cancel: CancellationToken,
    task: Option<tokio::task::JoinHandle<io::Result<()>>>,
}

async fn loopback_upstream(endpoint: &url::Url, port: u16) -> io::Result<Vec<SocketAddr>> {
    let denied = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "test proxy requires a loopback upstream",
        )
    };
    let addresses = match endpoint.host() {
        Some(url::Host::Ipv4(host)) => vec![SocketAddr::new(IpAddr::V4(host), port)],
        Some(url::Host::Ipv6(host)) => vec![SocketAddr::new(IpAddr::V6(host), port)],
        Some(url::Host::Domain("localhost")) => tokio::net::lookup_host(("localhost", port))
            .await?
            .collect(),
        // Non-special schemes, including PostgreSQL, preserve an IPv4 literal
        // as a domain string. Parse that literal without resolving other names.
        Some(url::Host::Domain(host)) => {
            vec![SocketAddr::new(host.parse().map_err(|_| denied())?, port)]
        }
        None => return Err(denied()),
    };
    if addresses.is_empty() || addresses.iter().any(|address| !address.ip().is_loopback()) {
        return Err(denied());
    }
    Ok(addresses)
}

impl AckCutTransport {
    pub async fn start(connection: &str, rules: Vec<AckCutRule>) -> io::Result<Self> {
        let mut ids = BTreeSet::new();
        if rules.is_empty()
            || rules
                .iter()
                .any(|rule| !rule.valid() || !ids.insert(rule.id))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid ACK cut rules",
            ));
        }
        let mut endpoint = url::Url::parse(connection).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid test PostgreSQL URL")
        })?;
        if !matches!(endpoint.scheme(), "postgres" | "postgresql") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid test PostgreSQL scheme",
            ));
        }
        if endpoint
            .query_pairs()
            .any(|(key, _)| matches!(key.as_ref(), "host" | "hostaddr" | "port"))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "test proxy address overrides are not supported",
            ));
        }
        let port = endpoint.port().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "explicit test PostgreSQL port required",
            )
        })?;
        let upstream = loopback_upstream(&endpoint, port).await?;
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        endpoint
            .set_host(Some("127.0.0.1"))
            .map_err(|_| io::Error::other("test proxy URL host failed"))?;
        endpoint
            .set_port(Some(listener.local_addr()?.port()))
            .map_err(|_| io::Error::other("test proxy URL port failed"))?;
        let parameters: Vec<_> = endpoint
            .query_pairs()
            .filter(|(key, _)| key != "sslmode")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        endpoint.set_query(None);
        endpoint
            .query_pairs_mut()
            .extend_pairs(parameters)
            .append_pair("sslmode", "disable");
        let pool = Arc::new(
            crate::pool::build_pool(endpoint.as_str())
                .map_err(|_| io::Error::other("test proxy pool failed"))?,
        );
        let rules: Arc<BTreeMap<_, _>> =
            Arc::new(rules.into_iter().map(|rule| (rule.id, rule)).collect());
        let control = Arc::new(TransportControl::default());
        let cut = Arc::new(AtomicUsize::new(0));
        let cancel = CancellationToken::new();
        let stopped = cancel.clone();
        let next = control.clone();
        let observed = cut.clone();
        let selected = rules.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            let mut session = 0_usize;
            let result = async { loop {
                tokio::select! {
                    _ = stopped.cancelled() => return Ok(()),
                    accepted = listener.accept() => {
                        let (downstream, _) = accepted?;
                        let upstream = tokio::select! {
                            _ = stopped.cancelled() => {
                                return Ok(());
                            },
                            connected = TcpStream::connect(upstream.as_slice()) => connected?,
                        };
                        downstream.set_nodelay(true)?;
                        upstream.set_nodelay(true)?;
                        session = session.checked_add(1)
                            .ok_or_else(|| io::Error::other("wire session counter exhausted"))?;
                        connections.spawn(relay(downstream, upstream, selected.clone(), next.clone(), observed.clone(), session));
                    },
                    finished = connections.join_next(), if !connections.is_empty() => {
                        let result = finished.ok_or_else(|| io::Error::other("missing wire task"))?
                            .map_err(|_| io::Error::other("wire task failed"))?;
                        match result {
                            Ok(()) => {},
                            Err(error) if matches!(error.kind(),
                                io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
                                | io::ErrorKind::ConnectionAborted | io::ErrorKind::UnexpectedEof
                                | io::ErrorKind::NotConnected) => {
                                // A deliberately terminated backend can race a
                                // rollback/write on this socket. Close only that
                                // connection; keep the independent observer,
                                // ownership session and real listener alive.
                                tracing::warn!(kind = ?error.kind(), "PostgreSQL test relay connection ended after peer disconnect");
                            },
                            Err(error) => return Err(error),
                        }
                    },
                }
            }}.await;
            // This path also runs on accept/connect/relay failure. No detached
            // forwarding task survives an early listener error.
            let closed = next.close(if result.is_ok() {
                AckHoldEnd::Shutdown
            } else {
                AckHoldEnd::IoFailure
            });
            connections.shutdown().await;
            result.and(closed)
        });
        let transport = Self {
            pool,
            connection: endpoint.to_string(),
            rules,
            control,
            cut,
            cancel,
            task: Some(task),
        };
        // Complete actual driver authentication before allowing a cut.
        drop(
            transport
                .pool
                .get()
                .await
                .map_err(|_| io::Error::other("test proxy authentication failed"))?,
        );
        Ok(transport)
    }
    pub fn connection_string(&self) -> &str {
        &self.connection
    }
    pub fn arm(&self, id: u8) -> io::Result<()> {
        if self.rules.get(&id).is_none_or(|rule| rule.hold) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown ACK cut rule",
            ));
        }
        self.control.arm(id, None).map(|_| ())
    }
    /// Select one already authenticated backend and one arm generation. PID
    /// must come from the caller's actual pooled connection, never a guess.
    pub fn arm_hold(&self, id: u8, backend_pid: i32) -> io::Result<AckHoldTicket> {
        if backend_pid <= 0 || self.rules.get(&id).is_none_or(|rule| !rule.hold) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid ACK hold rule or backend",
            ));
        }
        let (status, observed) = watch::channel(AckHoldStatus::Armed);
        let (release, receive) = oneshot::channel();
        let hold = Arc::new(HoldControl {
            backend_pid,
            status,
            release: Mutex::new(Some(receive)),
            abandoned: CancellationToken::new(),
        });
        let generation = self.control.arm(id, Some(hold.clone()))?;
        Ok(AckHoldTicket {
            generation,
            control: self.control.clone(),
            hold,
            status: observed,
            release: Some(release),
        })
    }
    pub fn cut_count(&self) -> usize {
        self.cut.load(Ordering::Acquire)
    }
    pub async fn shutdown(mut self) -> io::Result<()> {
        self.control.close(AckHoldEnd::Shutdown)?;
        self.cancel.cancel();
        self.task
            .take()
            .ok_or_else(|| io::Error::other("missing wire task"))?
            .await
            .map_err(|_| io::Error::other("wire task failed"))?
    }
}
impl Drop for AckCutTransport {
    fn drop(&mut self) {
        let _ = self.control.close(AckHoldEnd::Shutdown);
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn payload_length(header: &[u8; 4]) -> io::Result<usize> {
    usize::try_from(
        u32::from_be_bytes(*header).checked_sub(4).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid PostgreSQL frame")
        })?,
    )
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "unrepresentable PostgreSQL frame",
        )
    })
}
async fn relay(
    downstream: TcpStream,
    upstream: TcpStream,
    rules: Arc<BTreeMap<u8, AckCutRule>>,
    control: Arc<TransportControl>,
    cut: Arc<AtomicUsize>,
    session: usize,
) -> io::Result<()> {
    let (mut server, mut to_server) = upstream.into_split();
    let (mut client, mut to_client) = downstream.into_split();
    let matched = AtomicU64::new(0);
    let queried = AtomicU64::new(0);
    let backend_pid = AtomicI32::new(0);
    let requests = async {
        let mut length = [0_u8; 4];
        client.read_exact(&mut length).await?;
        let mut startup = vec![0; payload_length(&length)?];
        client.read_exact(&mut startup).await?;
        to_server.write_all(&length).await?;
        to_server.write_all(&startup).await?;
        loop {
            let tag = match client.read_u8().await {
                Ok(tag) => tag,
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
            };
            client.read_exact(&mut length).await?;
            let mut payload = vec![0; payload_length(&length)?];
            client.read_exact(&mut payload).await?;
            if let Some(arm) = control.selected()? {
                let eligible = arm
                    .hold
                    .as_ref()
                    .is_none_or(|hold| hold.backend_pid == backend_pid.load(Ordering::Acquire));
                if let Some(rule) = rules.get(&arm.id).filter(|_| eligible) {
                    if tag == b'B' && rule.matches(&payload) {
                        matched.store(arm.generation, Ordering::Release);
                    }
                    if tag == b'Q'
                        && matched.load(Ordering::Acquire) == arm.generation
                        && rule.following_query.as_deref() == Some(payload.as_slice())
                    {
                        queried.store(arm.generation, Ordering::Release);
                    }
                }
            }
            to_server.write_u8(tag).await?;
            to_server.write_all(&length).await?;
            to_server.write_all(&payload).await?;
        }
    };
    let answers = async {
        loop {
            let tag = match server.read_u8().await {
                Ok(tag) => tag,
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
            };
            let mut length = [0_u8; 4];
            server.read_exact(&mut length).await?;
            let mut payload = vec![0; payload_length(&length)?];
            server.read_exact(&mut payload).await?;
            if tag == b'K' {
                if payload.len() != 8 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid backend identity frame",
                    ));
                }
                // The remaining four bytes are the cancellation key. They are
                // forwarded unchanged and never retained in observation data.
                backend_pid.store(
                    i32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]),
                    Ordering::Release,
                );
            }
            if let Some(arm) = control.selected()?
                && let Some(rule) = rules.get(&arm.id)
            {
                let eligible =
                    rule.bind.is_none() || matched.load(Ordering::Acquire) == arm.generation;
                let query_observed = queried.load(Ordering::Acquire) == arm.generation;
                let acknowledged = eligible
                    && match rule.boundary {
                        AckBoundary::Begin => {
                            query_observed && tag == b'C' && payload == b"BEGIN\0"
                        }
                        AckBoundary::Commit => tag == b'C' && payload == b"COMMIT\0",
                        AckBoundary::IdleReady => tag == b'Z' && payload == b"I",
                    };
                if acknowledged {
                    if let Some(hold) = &arm.hold {
                        let observation = AckHoldObservation {
                            generation: arm.generation,
                            session,
                            backend_pid: backend_pid.load(Ordering::Acquire),
                            boundary: rule.boundary,
                            following_query_observed: query_observed,
                        };
                        if observation.backend_pid == hold.backend_pid {
                            if !control.held(observation)? {
                                return Ok(());
                            }
                            let receive = hold
                                .release
                                .lock()
                                .map_err(|_| io::Error::other("wire release poisoned"))?
                                .take()
                                .ok_or_else(|| io::Error::other("wire release already taken"))?;
                            tokio::select! {
                                biased;
                                _ = hold.abandoned.cancelled() => return Ok(()),
                                released = receive => released.map_err(|_| io::Error::other("wire release abandoned"))?,
                            }
                            to_client.write_u8(tag).await?;
                            to_client.write_all(&length).await?;
                            to_client.write_all(&payload).await?;
                            // Reset this session before publishing a free
                            // arm slot; otherwise a new generation's Bind
                            // could be erased by this old release.
                            matched.store(0, Ordering::Release);
                            queried.store(0, Ordering::Release);
                            if !control
                                .clear(arm.generation, Some(AckHoldStatus::Released(observation)))?
                            {
                                return Err(io::Error::new(
                                    io::ErrorKind::ConnectionAborted,
                                    "wire hold ended during release",
                                ));
                            }
                            continue;
                        }
                    } else if control.clear(arm.generation, None)? {
                        cut.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                            value.checked_add(1)
                        })
                        .map_err(|_| io::Error::other("ACK cut counter exhausted"))?;
                        to_client.shutdown().await?;
                        return Ok(());
                    }
                }
            }
            if tag == b'E' || (tag == b'C' && payload == b"ROLLBACK\0") {
                matched.store(0, Ordering::Release);
                queried.store(0, Ordering::Release);
            }
            to_client.write_u8(tag).await?;
            to_client.write_all(&length).await?;
            to_client.write_all(&payload).await?;
        }
    };
    // Dropping the other direction closes its actual upstream socket as well.
    // An EOF while held is not converted into an upstream-only zombie session.
    let result = tokio::select! { result=requests => result, result=answers => result };
    control.close_session(
        backend_pid.load(Ordering::Acquire),
        if result.is_ok() {
            AckHoldEnd::PeerClosed
        } else {
            AckHoldEnd::IoFailure
        },
    )?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn postgres_literal_loopbacks_resolve_without_domain_lookup() {
        for (connection, ip) in [
            ("postgres://test:private@127.0.0.1:5432/test", "127.0.0.1"),
            ("postgresql://test:private@127.0.0.2:5432/test", "127.0.0.2"),
            ("postgres://test:private@[::1]:5432/test", "::1"),
        ] {
            let endpoint = url::Url::parse(connection).unwrap();
            assert_eq!(
                loopback_upstream(&endpoint, 5432).await.unwrap(),
                vec![SocketAddr::new(ip.parse().unwrap(), 5432)]
            );
        }
    }

    #[test]
    fn bind_selectors_require_every_literal_and_exclude_forbidden_bytes() {
        let rule = AckCutRule::after_bind(
            1,
            vec![b"first\0".to_vec(), "ü🙂".as_bytes().to_vec()],
            vec![b"denied".to_vec()],
            AckBoundary::Commit,
        );
        assert!(rule.valid());
        assert!(rule.matches("first\0 data ü🙂".as_bytes()));
        assert!(!rule.matches(b"first\0 only"));
        assert!(!rule.matches("first\0 ü🙂 denied".as_bytes()));
    }

    #[tokio::test]
    async fn invalid_rules_schemes_and_nonloopback_upstreams_reject_before_connecting() {
        let local = "postgres://test:private@127.0.0.1:1/test";
        for (url, rules) in [
            (local, vec![]),
            (local, vec![AckCutRule::next_commit(0)]),
            (
                local,
                vec![AckCutRule::next_commit(1), AckCutRule::next_commit(1)],
            ),
            (
                local,
                vec![AckCutRule::after_bind(
                    1,
                    vec![],
                    vec![],
                    AckBoundary::Commit,
                )],
            ),
            (
                local,
                vec![AckCutRule::after_bind(
                    1,
                    vec![vec![]],
                    vec![],
                    AckBoundary::Commit,
                )],
            ),
            (
                local,
                vec![AckCutRule::after_bind(
                    1,
                    vec![b"x".to_vec()],
                    vec![vec![]],
                    AckBoundary::Commit,
                )],
            ),
            (
                local,
                vec![
                    AckCutRule::after_bind(1, vec![b"id".to_vec()], vec![], AckBoundary::Begin)
                        .hold(),
                ],
            ),
            (
                local,
                vec![
                    AckCutRule::after_bind(1, vec![b"id".to_vec()], vec![], AckBoundary::Begin)
                        .hold()
                        .following_query(vec![]),
                ],
            ),
            (
                local,
                vec![
                    AckCutRule::after_bind(1, vec![b"id".to_vec()], vec![], AckBoundary::Begin)
                        .hold()
                        .following_query(b"unterminated query".to_vec()),
                ],
            ),
            (
                local,
                vec![
                    AckCutRule::next_commit(1)
                        .hold()
                        .following_query(b"wrong boundary\0".to_vec()),
                ],
            ),
            ("http://127.0.0.1:1/test", vec![AckCutRule::next_commit(1)]),
            (
                "postgres://test:private@127.0.0.1:1/test?host=localhost",
                vec![AckCutRule::next_commit(1)],
            ),
            (
                "postgres://test:private@127.0.0.1:1/test?hostaddr=127.0.0.1",
                vec![AckCutRule::next_commit(1)],
            ),
            (
                "postgres://test:private@127.0.0.1:1/test?port=2",
                vec![AckCutRule::next_commit(1)],
            ),
            (
                "postgres://test:private@203.0.113.1:1/test",
                vec![AckCutRule::next_commit(1)],
            ),
            (
                "postgres://test:private@[2001:db8::1]:1/test",
                vec![AckCutRule::next_commit(1)],
            ),
        ] {
            let Err(error) = AckCutTransport::start(url, rules).await else {
                panic!("invalid wire fixture must not open a connection")
            };
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
    }
}
