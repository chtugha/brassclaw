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

#[path = "monty_ownership_policy.rs"]
mod policy;
use policy::{OwnershipControl, OwnershipRequest};
pub(crate) use policy::{OwnershipLimits, OwnershipSnapshot};
pub(crate) struct GlobalServiceConfig {
    pub(crate) boot: RootBoot,
    pub(crate) process: ProcessLimits,
    pub(crate) actor: ActorLimits,
    pub(crate) queue_capacity: u32,
    pub(crate) queue_bytes: usize,
    pub(crate) max_pending_settings: u32,
    pub(crate) max_retained_attempts: u32,
    pub(crate) live: LiveMontyTaskSettings,
    pub(crate) ownership: OwnershipLimits,
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
    requests: mpsc::UnboundedSender<OwnershipRequest>,
    control: OwnershipControl,
    closed: Arc<AtomicBool>,
}
impl GlobalOwnerCheck {
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) async fn check(&self) -> Result<(), OwnershipError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(OwnershipError::Lost);
        }
        let (answer, result) = oneshot::channel();
        let request = self.control.request(answer)?;
        let deadline = tokio::time::Instant::from_std(request.deadline);
        self.requests
            .send(request)
            .map_err(|_| OwnershipError::Connection)?;
        tokio::time::timeout_at(deadline, result)
            .await
            .map_err(|_| OwnershipError::Connection)?
            .map_err(|_| OwnershipError::Connection)?
    }
    pub(crate) fn snapshot(&self) -> Result<OwnershipSnapshot, OwnershipError> {
        self.control.snapshot()
    }
    pub(crate) fn publish(
        &self,
        expected: u64,
        revision: u64,
        limits: OwnershipLimits,
    ) -> Result<(), OwnershipError> {
        self.control.publish(expected, revision, limits)
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
        let control = OwnershipControl::new(config.live.current().revision, config.ownership)?;
        let guard =
            tokio::time::timeout(config.ownership.check_timeout, PgMontyOwner::acquire(pool))
                .await
                .map_err(|_| OwnershipError::Connection)??;
        // Register ownership before the first worker-start await. The
        // process registry also retains quarantine if this caller or supervisor
        // disappears; dropping a returned evidence object cannot unlock it.
        let guard = RetainedOwnership::register(guard)?;
        let closed = Arc::new(AtomicBool::new(false));
        let (requests, inbox) = mpsc::unbounded_channel();
        let check = GlobalOwnerCheck {
            requests,
            control,
            closed,
        };
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

// Each actual database enforces its own singleton. Retain unknown ownership
// without an arbitrary process-wide count; fail allocation before worker spawn.
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
        registry
            .try_reserve(1)
            .map_err(|_| OwnershipError::Connection)?;
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
    inbox: mpsc::UnboundedReceiver<OwnershipRequest>,
    stopping: watch::Receiver<bool>,
    check_port: GlobalOwnerCheck,
    ready: oneshot::Sender<Result<ServiceClient, GlobalOwnerError>>,
) -> Option<GlobalServiceExit> {
    let boot_timeout = config.ownership.check_timeout;
    let mut owner = match ServiceOwner::start_with_hosting_limits(
        &executable,
        config.boot,
        config.process,
        config.actor,
        brassclaw_monty_host::service::ServiceHostingLimits {
            admission: brassclaw_monty_host::service::AdmissionLimits {
                max_tasks: config.queue_capacity,
                max_bytes: config.queue_bytes,
            },
            max_pending_settings: config.max_pending_settings,
            max_retained_attempts: config.max_retained_attempts,
            actor: None,
            deadlines: None,
        },
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
                let release = tokio::time::timeout(boot_timeout, guard.release_quiescent()).await;
                if !matches!(release, Ok(Ok(()))) {
                    tracing::error!("global Monty boot ownership release was not acknowledged");
                }
            }
            check_port.closed.store(true, Ordering::Release);
            let _undelivered = ready.send(Err(GlobalOwnerError::Startup(Box::new(error))));
            return None;
        }
    };
    if let Err(error) = check(&guard, boot_timeout).await {
        owner.request_shutdown();
        let service = owner.join().await;
        let ownership = settle_ownership(guard, &service, &check_port.control).await;
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
    Some(
        supervise(
            guard,
            owner,
            inbox,
            stopping,
            check_port.closed,
            check_port.control,
        )
        .await,
    )
}

async fn check(guard: &RetainedOwnership, timeout: Duration) -> Result<(), OwnershipError> {
    tokio::time::timeout(timeout, guard.check())
        .await
        .map_err(|_| OwnershipError::Connection)?
}
enum Event {
    Service(Box<Result<ServiceExit, ServiceFailure>>),
    Request(Option<OwnershipRequest>),
    PolicyChanged,
    Heartbeat,
    Shutdown,
}
async fn supervise(
    guard: Arc<RetainedOwnership>,
    mut owner: ServiceOwner,
    mut requests: mpsc::UnboundedReceiver<OwnershipRequest>,
    mut shutdown: watch::Receiver<bool>,
    closed: Arc<AtomicBool>,
    control: OwnershipControl,
) -> GlobalServiceExit {
    let mut last_checked = std::time::Instant::now();
    let mut stopping = false;
    let mut ownership_failure = None;
    let service = loop {
        let policy = control.snapshot();
        let heartbeat_at = policy
            .as_ref()
            .ok()
            .and_then(|policy| last_checked.checked_add(policy.limits.heartbeat_interval));
        if !stopping && heartbeat_at.is_none() {
            ownership_failure = Some(OwnershipError::Connection);
            stopping = true;
            closed.store(true, Ordering::Release);
            requests.close();
            while let Ok(request) = requests.try_recv() {
                let _undelivered = request.answer.send(Err(OwnershipError::Lost));
            }
            owner.request_shutdown();
        }
        let event = tokio::select! {
            result = owner.join() => Event::Service(Box::new(result)),
            request = requests.recv(), if !stopping => Event::Request(request),
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(heartbeat_at.unwrap_or(last_checked))), if !stopping => Event::Heartbeat,
            _ = control.changed(), if !stopping => Event::PolicyChanged,
            _ = shutdown.changed(), if !stopping => Event::Shutdown,
        };
        match event {
            Event::Service(result) => break *result,
            Event::Request(Some(request)) => {
                let result = if std::time::Instant::now() >= request.deadline {
                    Err(OwnershipError::Deadline)
                } else {
                    tokio::time::timeout_at(
                        tokio::time::Instant::from_std(request.deadline),
                        guard.check(),
                    )
                    .await
                    .unwrap_or(Err(OwnershipError::Connection))
                };
                if let Err(error) = result
                    && !matches!(error, OwnershipError::Deadline)
                {
                    ownership_failure = Some(error);
                    stopping = true;
                }
                if result.is_ok() {
                    last_checked = std::time::Instant::now();
                }
                if closed.load(Ordering::Acquire) {
                    let _undelivered = request.answer.send(Err(OwnershipError::Lost));
                } else {
                    let _undelivered = request.answer.send(result);
                }
            }
            Event::Heartbeat => match policy.map(|policy| policy.limits.check_timeout) {
                Ok(timeout) => match check(&guard, timeout).await {
                    Ok(()) => last_checked = std::time::Instant::now(),
                    Err(error) => {
                        ownership_failure = Some(error);
                        stopping = true;
                    }
                },
                Err(error) => {
                    ownership_failure = Some(error);
                    stopping = true;
                }
            },
            Event::PolicyChanged => {}
            Event::Shutdown | Event::Request(None) => stopping = true,
        }
        if stopping {
            closed.store(true, Ordering::Release);
            requests.close();
            while let Ok(request) = requests.try_recv() {
                let _undelivered = request.answer.send(Err(OwnershipError::Lost));
            }
            owner.request_shutdown();
        }
    };
    closed.store(true, Ordering::Release);
    requests.close();
    while let Ok(request) = requests.try_recv() {
        let _undelivered = request.answer.send(Err(OwnershipError::Lost));
    }
    let ownership = settle_ownership(guard, &service, &control).await;
    GlobalServiceExit {
        service,
        ownership,
        ownership_failure,
    }
}

async fn settle_ownership(
    guard: Arc<RetainedOwnership>,
    service: &Result<ServiceExit, ServiceFailure>,
    control: &OwnershipControl,
) -> OwnershipSettlement {
    if service.as_ref().is_ok_and(|exit| {
        exit.transport.as_ref().is_ok_and(|transport| {
            transport.exit_status.is_some()
                && transport.reap_error.is_none()
                && transport.containment_error.is_none()
        })
    }) {
        let Ok(policy) = control.snapshot() else {
            return OwnershipSettlement::Quarantined(guard);
        };
        OwnershipSettlement::ReleaseAttempt(
            tokio::time::timeout(policy.limits.check_timeout, guard.release_quiescent())
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
