//! Opt-in physical retirement for an original IO exchange whose owner survives
//! cancellation/panic of its SQL body. This never retries application SQL.
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use super::TransactionClient;

#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("original PostgreSQL transaction retirement is unresolved")]
pub struct RetirementUnresolved;

struct BackendIdentity {
    pid: i32,
    // Epoch text avoids timezone changes and retains PostgreSQL's full timestamp
    // precision without introducing a second timestamp codec.
    started: String,
}

struct RetirementJob {
    handle: Option<tokio::task::JoinHandle<Result<(), RetirementUnresolved>>>,
    result: Option<Result<(), RetirementUnresolved>>,
}
impl Drop for RetirementJob {
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.abort();
        }
    }
}

/// One original checkout, at most one bounded retirement job. The caller must
/// publish this custody before acquiring, join the actual SQL body first, then
/// join retirement before releasing the original task or shutting PostgreSQL
/// down. Drop alone is never a server acknowledgement.
pub struct TransactionRetirement {
    pool: crate::PgPool,
    timeout: Duration,
    acquired: AtomicBool,
    confirmed: Arc<AtomicBool>,
    identity: Arc<OnceLock<BackendIdentity>>,
    job: Mutex<Option<Arc<tokio::sync::Mutex<RetirementJob>>>>,
}
impl TransactionRetirement {
    pub fn new(pool: crate::PgPool, timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            pool,
            timeout,
            acquired: AtomicBool::new(false),
            confirmed: Arc::new(AtomicBool::new(true)),
            identity: Arc::new(OnceLock::new()),
            job: Mutex::new(None),
        })
    }

    /// Acquire at most one original checkout inside the caller's SQL budget.
    /// Capture its backend before BEGIN/effects, then retain this custody until
    /// the SQL body and any physical retirement have actually joined.
    pub async fn acquire(self: &Arc<Self>) -> Result<TransactionClient, RetirementUnresolved> {
        if self.acquired.swap(true, Ordering::AcqRel) {
            return Err(RetirementUnresolved);
        }
        let client = self.pool.get().await.map_err(|_| RetirementUnresolved)?;
        self.confirmed.store(false, Ordering::Release);
        Ok(TransactionClient::with_retirement(client, self.clone()))
    }

    /// This read cannot share the claim query, which may block before returning
    /// any row. It holds no second checkout and stays inside the SQL budget.
    pub async fn capture_backend(
        &self,
        client: &TransactionClient,
    ) -> Result<(), RetirementUnresolved> {
        let Some(owner) = client.retirement.as_ref() else {
            return Err(RetirementUnresolved);
        };
        if !std::ptr::eq(owner.as_ref(), self) {
            return Err(RetirementUnresolved);
        }
        capture_identity(client, &self.identity).await
    }

    pub fn is_confirmed(&self) -> bool {
        self.confirmed.load(Ordering::Acquire)
    }

    pub async fn join(&self) -> Result<(), RetirementUnresolved> {
        let job = self
            .job
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(job) = job {
            let mut job = job.lock().await;
            if job.result.is_none() {
                let result = match job.handle.as_mut() {
                    Some(handle) => handle.await.unwrap_or(Err(RetirementUnresolved)),
                    None => Err(RetirementUnresolved),
                };
                job.result = Some(result);
                job.handle = None;
            }
            job.result.unwrap_or(Err(RetirementUnresolved))?;
        }
        self.is_confirmed()
            .then_some(())
            .ok_or(RetirementUnresolved)
    }

    pub(super) fn recycled(&self) {
        self.confirmed.store(true, Ordering::Release);
    }

    pub(super) fn retire(&self, client: deadpool_postgres::ClientWrapper) {
        // The wrapper is already taken out of deadpool. Neither this session nor
        // its cancellation token can ever be recycled into unrelated work.
        self.confirmed.store(false, Ordering::Release);
        let pool = self.pool.clone();
        let Some(deadline) = tokio::time::Instant::now().checked_add(self.timeout) else {
            drop(client);
            return;
        };
        let identity = self.identity.clone();
        let confirmed = self.confirmed.clone();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            // No runtime is an explicit unresolved retirement, never a detached
            // job or a successful cleanup. Dropping still removes the socket.
            drop(client);
            return;
        };
        let handle = runtime.spawn(async move {
            let result =
                tokio::time::timeout_at(deadline, retire_original(client, &pool, &identity))
                    .await
                    .map_err(|_| RetirementUnresolved)?;
            if result.is_ok() {
                confirmed.store(true, Ordering::Release);
            }
            result
        });
        // Publication is synchronous with Drop; the SQL task's join therefore
        // happens after its sole retirement handle has been retained.
        let mut job = self
            .job
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *job = Some(Arc::new(tokio::sync::Mutex::new(RetirementJob {
            handle: Some(handle),
            result: None,
        })));
    }
}

async fn capture_identity(
    client: &deadpool_postgres::ClientWrapper,
    identity: &OnceLock<BackendIdentity>,
) -> Result<(), RetirementUnresolved> {
    let row = client.query_one(
        "SELECT pid,extract(epoch FROM backend_start)::text FROM pg_stat_activity WHERE pid=pg_backend_pid()", &[],
    ).await.map_err(|_| RetirementUnresolved)?;
    identity
        .set(BackendIdentity {
            pid: row.get(0),
            started: row.get(1),
        })
        .map_err(|_| RetirementUnresolved)
}

async fn retire_original(
    client: deadpool_postgres::ClientWrapper,
    pool: &crate::PgPool,
    identity: &OnceLock<BackendIdentity>,
) -> Result<(), RetirementUnresolved> {
    // If the body's first read was cancelled, it has not issued BEGIN/effects.
    // Obtain only its original session identity, never application work.
    if identity.get().is_none() {
        capture_identity(&client, identity).await?;
    }
    let original = identity.get().ok_or(RetirementUnresolved)?;
    // The token retains the verified endpoint/SSL mode from the shared pool
    // policy. Reuse its certificate-validating connector even for cancellation;
    // local plaintext tokens do not invoke a TLS handshake.
    let _sent = crate::connection::cancel_query(client.cancel_token()).await;
    // CancelRequest has no acknowledgement. Close the original driver/socket,
    // then use a new checkout for actual server acknowledgement. Pool-one works
    // because no original checkout/permit remains at this point.
    drop(client);
    let observer =
        TransactionClient::from_client(pool.get().await.map_err(|_| RetirementUnresolved)?);
    loop {
        let live: bool = observer.query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND extract(epoch FROM backend_start)::text=$2)",
            &[&original.pid, &original.started],
        ).await.map_err(|_| RetirementUnresolved)?.get(0);
        if !live {
            return Ok(());
        }
        tokio::task::yield_now().await;
    }
}
