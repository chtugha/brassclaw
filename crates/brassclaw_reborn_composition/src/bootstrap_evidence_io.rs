//! Bounded immutable evidence I/O under the existing acknowledged technical
//! ResourceStore deadline. No task execution, queue or approval authority.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_pg::PgPool;
use tokio_postgres::IsolationLevel;
use uuid::Uuid;
/// Do not recycle a connection whose transaction response was lost/cancelled.
/// The normal acknowledged path still uses the existing pool. Detaching only
/// the uncertain connection prevents a queued BEGIN from leaking into readback.
pub(crate) struct EvidenceClient {
    inner: Option<brassclaw_pg::PgClient>,
    recyclable: bool,
}
impl EvidenceClient {
    pub(crate) async fn acquire(pool: &PgPool) -> Result<Self, RebornBuildError> {
        Ok(Self::from_client(pool.get().await.map_err(invalid)?))
    }
    /// Retain the caller's existing connection error classification while
    /// applying the same uncertain-transaction custody and drop discipline.
    pub(crate) fn from_client(client: brassclaw_pg::PgClient) -> Self {
        Self {
            inner: Some(client),
            recyclable: false,
        }
    }
    pub(crate) fn recycle_after_commit(&mut self) {
        self.recyclable = true;
    }
}
impl std::ops::Deref for EvidenceClient {
    type Target = brassclaw_pg::PgClient;
    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("owned evidence connection")
    }
}
impl std::ops::DerefMut for EvidenceClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("owned evidence connection")
    }
}
impl Drop for EvidenceClient {
    fn drop(&mut self) {
        if !self.recyclable {
            if let Some(client) = self.inner.take() {
                drop(brassclaw_pg::PgClient::take(client));
            }
        }
    }
}

pub(crate) async fn retain_new_intent(
    pool: &PgPool,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    id: Uuid,
    bytes: &str,
) -> Result<(), RebornBuildError> {
    let write = bounded(storage, async {
        let mut client = EvidenceClient::acquire(pool).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(invalid)?;
        if !crate::bootstrap_evidence::retain_intent(&tx, id, bytes).await? {
            return Err(invalid(
                "original intent already retained; reconcile without replay",
            ));
        }
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        Ok(())
    })
    .await;
    // Also inspect the original on a lost response. No dispatch follows an
    // unacknowledged new-intent write, even when its row is now visible.
    let readback = require_record(pool, storage, id, bytes).await;
    write?;
    readback
}
pub(crate) async fn read_record(
    pool: &PgPool,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    id: Uuid,
) -> Result<Option<String>, RebornBuildError> {
    bounded(storage, async {
        let mut client = EvidenceClient::acquire(pool).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        let bytes = crate::bootstrap_evidence::read_evidence(&tx, id).await?;
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        Ok(bytes)
    })
    .await
}
pub(crate) async fn require_record(
    pool: &PgPool,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    id: Uuid,
    bytes: &str,
) -> Result<(), RebornBuildError> {
    if read_record(pool, storage, id).await?.as_deref() != Some(bytes) {
        return Err(invalid("original primitive record readback differs"));
    }
    Ok(())
}
pub(crate) async fn retain_record(
    pool: &PgPool,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    id: Uuid,
    bytes: &str,
) -> Result<(), RebornBuildError> {
    let write = bounded(storage, async {
    let mut client = EvidenceClient::acquire(pool).await?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    tx.execute("INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(evidence_id) DO NOTHING",
        &[&id,&bytes,&crate::bootstrap_evidence::checksum(bytes)]).await.map_err(invalid)?;
    tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        Ok(())
    }).await;
    // Read back the exact original regardless of whether COMMIT's response was
    // received. A timeout/connection failure is not evidence of no effect.
    match require_record(pool, storage, id, bytes).await {
        Ok(()) => Ok(()),
        Err(_) if write.is_err() => Err(invalid(
            "original observation write unresolved; reconcile without replay",
        )),
        Err(error) => Err(error),
    }
}

/// Existing acknowledged technical store deadline, captured once for this
/// logical metadata operation. It is independent of VM executing-time budgets
/// and never replaces the qualification's historical preparation/configuration.
pub(crate) async fn bounded<T>(
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    operation: impl std::future::Future<Output = Result<T, RebornBuildError>>,
) -> Result<T, RebornBuildError> {
    let selected = storage
        .snapshot()
        .map_err(invalid)?
        .validate()
        .map_err(invalid)?;
    tokio::time::timeout(selected.timeout, operation)
        .await
        .map_err(|_| invalid("original observation I/O deadline elapsed; outcome unresolved"))?
}
