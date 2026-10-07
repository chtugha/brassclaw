//! Database-instance ownership bound to actual global service settlement.
//! Cutover candidate: application ABI/resource/catalogue gates still apply.
use crate::monty_instance_owner::{OwnershipError, PgMontyOwner};
use brassclaw_monty_host::{
    process::{ProcessLimits, RootBoot},
    service::{ServiceClient, ServiceExit, ServiceFailure, ServiceOwner},
    transport_actor::ActorLimits,
};
use brassclaw_pg::PgPool;
use brassclaw_resources::LiveMontyTaskSettings;
use std::{
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

const OWNER_CHECK_BOUND: Duration = Duration::from_secs(2);
pub(crate) struct GlobalServiceConfig {
    pub(crate) boot: RootBoot,
    pub(crate) process: ProcessLimits,
    pub(crate) actor: ActorLimits,
    pub(crate) queue_capacity: usize,
    pub(crate) live: LiveMontyTaskSettings,
}

#[derive(thiserror::Error)]
pub(crate) enum GlobalOwnerError {
    #[error(transparent)]
    Ownership(#[from] OwnershipError),
    #[error(transparent)]
    Startup(Box<brassclaw_monty_host::transport_actor::StartError>),
    #[error("global Monty lost instance ownership during boot: {failure}")]
    BootOwnershipLost {
        failure: OwnershipError,
        evidence: Box<GlobalServiceExit>,
    },
    #[error("global Monty owner supervision failed")]
    Join,
}
impl std::fmt::Debug for GlobalOwnerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut value = formatter.debug_struct("GlobalOwnerError");
        value.field("kind", &self.to_string());
        if let Self::BootOwnershipLost { evidence, .. } = self {
            value.field("service_returned", &evidence.service.is_ok());
        }
        value.finish_non_exhaustive()
    }
}

/// Private ownership checks, never a Tool grant or serializable claim.
#[derive(Clone)]
pub(crate) struct GlobalOwnerCheck {
    requests: mpsc::Sender<oneshot::Sender<Result<(), OwnershipError>>>,
    closed: Arc<AtomicBool>,
}
impl GlobalOwnerCheck {
    pub(crate) async fn check(&self) -> Result<(), OwnershipError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(OwnershipError::Lost);
        }
        let (answer, result) = oneshot::channel();
        self.requests
            .try_send(answer)
            .map_err(|_| OwnershipError::Connection)?;
        tokio::time::timeout(OWNER_CHECK_BOUND, result)
            .await
            .map_err(|_| OwnershipError::Connection)?
            .map_err(|_| OwnershipError::Connection)?
    }
}

/// Unknown transport/service quiescence retains ownership as supervisor evidence.
/// Never drop a quarantined guard to implement automatic fatal-task replay.
pub(crate) enum OwnershipSettlement {
    ReleaseAttempt(Result<(), OwnershipError>),
    Quarantined(Arc<RetainedOwnership>),
}
pub(crate) struct GlobalServiceExit {
    pub(crate) service: Result<ServiceExit, ServiceFailure>,
    pub(crate) ownership: OwnershipSettlement,
    pub(crate) ownership_failure: Option<OwnershipError>,
}

pub(crate) struct GlobalMontyOwner {
    service: ServiceClient,
    check: GlobalOwnerCheck,
    shutdown: watch::Sender<bool>,
    join: Option<JoinHandle<Option<GlobalServiceExit>>>,
}
impl GlobalMontyOwner {
    pub(crate) async fn start(
        pool: &PgPool,
        executable: &Path,
        config: GlobalServiceConfig,
    ) -> Result<Self, GlobalOwnerError> {
        let guard = tokio::time::timeout(OWNER_CHECK_BOUND, PgMontyOwner::acquire(pool))
            .await
            .map_err(|_| OwnershipError::Connection)??;
        // Register ownership before the first worker-start await. The bounded
        // process registry also retains quarantine if this caller or supervisor
        // disappears; dropping a returned evidence object cannot unlock it.
        let guard = RetainedOwnership::register(guard)?;
        let closed = Arc::new(AtomicBool::new(false));
        let (requests, inbox) = mpsc::channel(8);
        let check = GlobalOwnerCheck { requests, closed };
        let (shutdown, stopping) = watch::channel(false);
        let (ready, started) = oneshot::channel();
        let executable = executable.to_owned();
        let join = tokio::spawn(bootstrap(
            guard,
            executable,
            config,
            inbox,
            stopping,
            check.clone(),
            ready,
        ));
        let service = started.await.map_err(|_| GlobalOwnerError::Join)??;
        Ok(Self {
            service,
            check,
            shutdown,
            join: Some(join),
        })
    }
    pub(crate) fn client(&self) -> ServiceClient {
        self.service.clone()
    }
    pub(crate) fn ownership_check(&self) -> GlobalOwnerCheck {
        self.check.clone()
    }
    pub(crate) fn request_shutdown(&self) {
        self.service.close_admission();
        self.check.closed.store(true, Ordering::Release);
        self.shutdown.send_replace(true);
    }
    pub(crate) async fn join(&mut self) -> Result<GlobalServiceExit, GlobalOwnerError> {
        let result = self.join.as_mut().ok_or(GlobalOwnerError::Join)?.await;
        self.join.take();
        result
            .map_err(|_| GlobalOwnerError::Join)?
            .ok_or(GlobalOwnerError::Join)
    }
}
impl Drop for GlobalMontyOwner {
    fn drop(&mut self) {
        self.request_shutdown();
    }
}

// One product normally has one database. Bound diagnostic ownership across
// multiple in-process test/embedded instances; reject before spawning a worker.
const MAX_RETAINED_OWNERS: usize = 32;
static RETAINED_OWNERS: OnceLock<Mutex<Vec<Arc<RetainedOwnership>>>> = OnceLock::new();

pub(crate) struct RetainedOwnership {
    guard: tokio::sync::Mutex<Option<PgMontyOwner>>,
}
impl RetainedOwnership {
    fn register(guard: PgMontyOwner) -> Result<Arc<Self>, OwnershipError> {
        let mut registry = RETAINED_OWNERS
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|_| OwnershipError::Connection)?;
        if registry.len() >= MAX_RETAINED_OWNERS {
            return Err(OwnershipError::Connection);
        }
        let retained = Arc::new(Self {
            guard: tokio::sync::Mutex::new(Some(guard)),
        });
        registry.push(retained.clone());
        Ok(retained)
    }
    pub(crate) async fn check(&self) -> Result<(), OwnershipError> {
        let guard = self.guard.lock().await;
        guard.as_ref().ok_or(OwnershipError::Lost)?.check().await
    }
    // Only actual worker exit plus settled host futures calls this method.
    // A timeout during unlock may close that already-quiescent connection.
    async fn release_quiescent(self: &Arc<Self>) -> Result<(), OwnershipError> {
        let guard = self.guard.lock().await.take().ok_or(OwnershipError::Lost)?;
        // Remove after extracting the connection. Quiescence has already been
        // proven, so cancellation during the explicit unlock is safe.
        {
            let mut registry = RETAINED_OWNERS
                .get_or_init(Mutex::default)
                .lock()
                .map_err(|_| OwnershipError::Connection)?;
            registry.retain(|owner| !Arc::ptr_eq(owner, self));
        }
        guard.release().await
    }
}

async fn bootstrap(
    guard: Arc<RetainedOwnership>,
    executable: std::path::PathBuf,
    config: GlobalServiceConfig,
    inbox: mpsc::Receiver<oneshot::Sender<Result<(), OwnershipError>>>,
    stopping: watch::Receiver<bool>,
    check_port: GlobalOwnerCheck,
    ready: oneshot::Sender<Result<ServiceClient, GlobalOwnerError>>,
) -> Option<GlobalServiceExit> {
    let mut owner = match ServiceOwner::start_with_live_settings(
        &executable,
        config.boot,
        config.process,
        config.actor,
        config.queue_capacity,
        config.live,
    )
    .await
    {
        Ok(owner) => owner,
        Err(error) => {
            // Actor failures precede process creation. A worker failure only
            // proves termination with an actual reaped status, or before spawn.
            use brassclaw_monty_host::{process::ProcessFailure, transport_actor::StartError};
            let quiescent = match &error {
                StartError::Actor(_) => true,
                StartError::Worker(error) => {
                    matches!(
                        error.kind,
                        ProcessFailure::InvalidLimits
                            | ProcessFailure::InvalidExecutable
                            | ProcessFailure::Spawn
                    ) || (error.exit_status.is_some()
                        && error.containment_error.is_none()
                        && error.reap_error.is_none())
                }
            };
            if quiescent {
                let release =
                    tokio::time::timeout(OWNER_CHECK_BOUND, guard.release_quiescent()).await;
                if !matches!(release, Ok(Ok(()))) {
                    tracing::error!("global Monty boot ownership release was not acknowledged");
                }
            }
            check_port.closed.store(true, Ordering::Release);
            let _undelivered = ready.send(Err(GlobalOwnerError::Startup(Box::new(error))));
            return None;
        }
    };
    if let Err(error) = check(&guard).await {
        owner.request_shutdown();
        let service = owner.join().await;
        let ownership = settle_ownership(guard, &service).await;
        check_port.closed.store(true, Ordering::Release);
        let _undelivered = ready.send(Err(GlobalOwnerError::BootOwnershipLost {
            failure: error,
            evidence: Box::new(GlobalServiceExit {
                service,
                ownership,
                ownership_failure: Some(error),
            }),
        }));
        return None;
    }
    if ready.send(Ok(owner.client())).is_err()
        || *stopping.borrow()
        || stopping.has_changed().is_err()
    {
        // Abandoned boot still owns its worker and database through real exit.
        check_port.closed.store(true, Ordering::Release);
        owner.request_shutdown();
    }
    Some(supervise(guard, owner, inbox, stopping, check_port.closed).await)
}

async fn check(guard: &RetainedOwnership) -> Result<(), OwnershipError> {
    tokio::time::timeout(OWNER_CHECK_BOUND, guard.check())
        .await
        .map_err(|_| OwnershipError::Connection)?
}
enum Event {
    Service(Box<Result<ServiceExit, ServiceFailure>>),
    Request(Option<oneshot::Sender<Result<(), OwnershipError>>>),
    Heartbeat,
    Shutdown,
}
async fn supervise(
    guard: Arc<RetainedOwnership>,
    mut owner: ServiceOwner,
    mut requests: mpsc::Receiver<oneshot::Sender<Result<(), OwnershipError>>>,
    mut shutdown: watch::Receiver<bool>,
    closed: Arc<AtomicBool>,
) -> GlobalServiceExit {
    let mut heartbeat = tokio::time::interval(OWNER_CHECK_BOUND);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut stopping = false;
    let mut ownership_failure = None;
    let service = loop {
        let event = tokio::select! {
            result = owner.join() => Event::Service(Box::new(result)),
            request = requests.recv(), if !stopping => Event::Request(request),
            _ = heartbeat.tick(), if !stopping => Event::Heartbeat,
            _ = shutdown.changed(), if !stopping => Event::Shutdown,
        };
        match event {
            Event::Service(result) => break *result,
            Event::Request(Some(answer)) => {
                let result = check(&guard).await;
                if let Err(error) = result {
                    ownership_failure = Some(error);
                    stopping = true;
                }
                if closed.load(Ordering::Acquire) {
                    let _undelivered = answer.send(Err(OwnershipError::Lost));
                } else {
                    let _undelivered = answer.send(result);
                }
            }
            Event::Heartbeat => {
                if let Err(error) = check(&guard).await {
                    ownership_failure = Some(error);
                    stopping = true;
                }
            }
            Event::Shutdown | Event::Request(None) => stopping = true,
        }
        if stopping {
            closed.store(true, Ordering::Release);
            requests.close();
            while let Ok(answer) = requests.try_recv() {
                let _undelivered = answer.send(Err(OwnershipError::Lost));
            }
            owner.request_shutdown();
        }
    };
    closed.store(true, Ordering::Release);
    requests.close();
    while let Ok(answer) = requests.try_recv() {
        let _undelivered = answer.send(Err(OwnershipError::Lost));
    }
    let ownership = settle_ownership(guard, &service).await;
    GlobalServiceExit {
        service,
        ownership,
        ownership_failure,
    }
}

async fn settle_ownership(
    guard: Arc<RetainedOwnership>,
    service: &Result<ServiceExit, ServiceFailure>,
) -> OwnershipSettlement {
    if service.as_ref().is_ok_and(|exit| {
        exit.transport.as_ref().is_ok_and(|transport| {
            transport.exit_status.is_some()
                && transport.reap_error.is_none()
                && transport.containment_error.is_none()
        })
    }) {
        OwnershipSettlement::ReleaseAttempt(
            tokio::time::timeout(OWNER_CHECK_BOUND, guard.release_quiescent())
                .await
                .unwrap_or(Err(OwnershipError::Connection)),
        )
    } else {
        OwnershipSettlement::Quarantined(guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_quarantine_evidence_drop_does_not_unlock_instance() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let guard = PgMontyOwner::acquire(&rig.pool).await.unwrap();
        let retained = RetainedOwnership::register(guard).unwrap();
        let receipt = OwnershipSettlement::Quarantined(retained.clone());
        let weak = Arc::downgrade(&retained);
        drop(retained);
        drop(receipt);
        assert!(matches!(
            PgMontyOwner::acquire(&rig.pool).await,
            Err(OwnershipError::AlreadyOwned)
        ));
        let retained = weak
            .upgrade()
            .expect("registry must own abandoned quarantine");
        retained.check().await.unwrap();
        // This fixture started no worker or host future; quiescence is known.
        retained.release_quiescent().await.unwrap();
        PgMontyOwner::acquire(&rig.pool)
            .await
            .unwrap()
            .release()
            .await
            .unwrap();
    }
}
