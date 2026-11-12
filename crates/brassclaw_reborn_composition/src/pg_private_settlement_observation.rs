//! Bounded test witnesses at actual existing settlement boundaries. No observer
//! polls execution, acquires a resource, or supplies settlement evidence.
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::Instant;

#[derive(Clone, Copy)]
pub(crate) enum SettlementPhase {
    ReconcileNativeStarted,
    ReconcileNativeAcknowledged,
    ReconcileChildrenStarted,
    ReconcileChildrenAcknowledged,
    ReportStarted,
    ReportAcknowledged,
    ReportReused,
    AcquireStarted,
    AcquireAcknowledged,
    BeginStarted,
    BeginAcknowledged,
    AddressStarted,
    AddressAcknowledged,
    RefsStarted,
    RefsAcknowledged,
    EncodeStarted,
    EncodeCompleted,
    CasStarted,
    CasAcknowledged,
    TerminalStarted,
    TerminalAcknowledged,
    CommitStarted,
    CommitAcknowledged,
    FactoryCompleted,
}
const PHASES: [&str; 24] = [
    "reconcile_native_started",
    "reconcile_native_acknowledged",
    "reconcile_children_started",
    "reconcile_children_acknowledged",
    "report_started",
    "report_acknowledged",
    "report_reused",
    "acquire_started",
    "acquire_acknowledged",
    "begin_started",
    "begin_acknowledged",
    "address_started",
    "address_acknowledged",
    "refs_started",
    "refs_acknowledged",
    "encode_started",
    "encode_completed",
    "cas_started",
    "cas_acknowledged",
    "terminal_started",
    "terminal_acknowledged",
    "commit_started",
    "commit_acknowledged",
    "factory_completed",
];

#[derive(Default)]
pub(super) struct SettlementObservation {
    sequence: AtomicU64,
    latest: Mutex<Option<Arc<Exchange>>>,
}
impl SettlementObservation {
    pub(super) fn start(&self) -> SettlementScope {
        let exchange = Arc::new(Exchange {
            sequence: self
                .sequence
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_add(1),
            started: Instant::now(),
            last: AtomicUsize::new(PHASES.len()),
            times: std::array::from_fn(|_| OnceLock::new()),
            client_id: OnceLock::new(),
            client_scope_ended: OnceLock::new(),
            factory_scope_ended: OnceLock::new(),
        });
        // Observer-only pointer publication; never wait on a diagnostic reader.
        if let Ok(mut latest) = self.latest.try_lock() {
            *latest = Some(exchange.clone());
        }
        SettlementScope(exchange)
    }
    pub(super) fn snapshot(&self) -> Value {
        let latest = match self.latest.try_lock() {
            Ok(latest) => latest.clone(),
            Err(_) => return json!({"observation":"settlement_observer_unavailable"}),
        };
        let mut value = latest.map_or_else(
            || json!({"observation":"no_published_settlement_exchange"}),
            |exchange| exchange.snapshot(),
        );
        value["started_exchange_count_observed"] = json!(self.sequence.load(Ordering::Relaxed));
        value
    }
}
struct Exchange {
    sequence: u64,
    started: Instant,
    last: AtomicUsize,
    times: [OnceLock<u64>; PHASES.len()],
    client_id: OnceLock<String>,
    client_scope_ended: OnceLock<u64>,
    factory_scope_ended: OnceLock<u64>,
}
impl Exchange {
    fn elapsed(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    fn note(&self, phase: SettlementPhase) {
        let index = phase as usize;
        let _ = self.times[index].set(self.elapsed());
        self.last.store(index, Ordering::Release);
    }
    fn snapshot(&self) -> Value {
        let last = self.last.load(Ordering::Acquire);
        let elapsed = self.elapsed();
        let acquired = self.times[SettlementPhase::AcquireAcknowledged as usize].get();
        let ended = self.client_scope_ended.get();
        let boundaries = PHASES
            .iter()
            .enumerate()
            .filter_map(|(index, phase)| {
                self.times[index]
                    .get()
                    .map(|millis| json!({"phase":phase,"elapsed_millis":millis}))
            })
            .collect::<Vec<_>>();
        // Independently observed monotone facts, not an atomic state snapshot.
        // A failed try_lock publication never relabels an older exchange latest.
        json!({"scope":"last_published_factory_settlement_exchange", "sequence":self.sequence,
            "snapshot":"passive_non_atomic",
            "phase":PHASES.get(last).copied().unwrap_or("not_started"),
            "elapsed_millis":elapsed, "boundaries":boundaries,
            "client_id":self.client_id.get(),
            "client_acquired_at_millis":acquired,
            "client_scope_ended_at_millis":ended,
            "client_scope_millis":acquired.map(|start| ended.copied().unwrap_or(elapsed).saturating_sub(*start)),
            "factory_scope_ended_at_millis":self.factory_scope_ended.get(),
            "commit_acknowledged":self.times[SettlementPhase::CommitAcknowledged as usize].get().is_some()})
    }
}

pub(crate) struct SettlementScope(Arc<Exchange>);
impl SettlementScope {
    pub(crate) fn note(&self, phase: SettlementPhase) {
        self.0.note(phase);
    }
    pub(crate) fn client_scope(&self) -> SettlementClientScope {
        SettlementClientScope {
            exchange: self.0.clone(),
            acquired: false,
        }
    }
}
impl Drop for SettlementScope {
    fn drop(&mut self) {
        let _ = self.0.factory_scope_ended.set(self.0.elapsed());
    }
}
pub(crate) struct SettlementClientScope {
    exchange: Arc<Exchange>,
    acquired: bool,
}
impl SettlementClientScope {
    pub(crate) fn acquired(&mut self, client: &brassclaw_pg::PgClient) {
        let _ = self
            .exchange
            .client_id
            .set(brassclaw_pg::PgClient::id(client).to_string());
        self.acquired = true;
        self.exchange.note(SettlementPhase::AcquireAcknowledged);
    }
}
impl Drop for SettlementClientScope {
    fn drop(&mut self) {
        if self.acquired {
            // Declared before the actual pooled client: this runs after Rust
            // disposes that guard, not after a PostgreSQL rollback/close ACK.
            let _ = self
                .exchange
                .client_scope_ended
                .set(self.exchange.elapsed());
        }
    }
}
