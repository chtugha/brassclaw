//! Bounded immutable evidence I/O under the existing acknowledged technical
//! ResourceStore deadline. No task execution, queue or approval authority.
/// Do not recycle a connection whose transaction response was lost/cancelled.
/// The normal acknowledged path still uses the existing pool. Detaching only
/// the uncertain connection prevents a queued BEGIN from leaking into readback.
pub(crate) use crate::pg_transaction_client::TransactionClient as EvidenceClient;
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_pg::PgPool;
use tokio_postgres::IsolationLevel;
use uuid::Uuid;
impl EvidenceClient {
    pub(crate) async fn acquire(pool: &PgPool) -> Result<Self, RebornBuildError> {
        Ok(Self::from_client(pool.get().await.map_err(invalid)?))
    }
}

pub(crate) async fn retain_new_intent(
    pool: &PgPool,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    id: Uuid,
    bytes: &str,
) -> Result<(), RebornBuildError> {
    let write = bounded_for(storage, "evidence_intent_write", id, async {
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
    bounded_for(storage, "evidence_record_read", id, async {
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
    let write = bounded_for(storage, "evidence_record_write", id, async {
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
    bounded_operation(storage, None, operation).await
}

/// Closed operation identity only. Callers supply a static stage and an existing
/// non-secret component/evidence UUID, never payload, actor or connection data.
pub(crate) async fn bounded_for<T>(
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    stage: &'static str,
    identity: Uuid,
    operation: impl std::future::Future<Output = Result<T, RebornBuildError>>,
) -> Result<T, RebornBuildError> {
    bounded_operation(storage, Some((stage, identity)), operation).await
}

async fn bounded_operation<T>(
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    context: Option<(&'static str, Uuid)>,
    operation: impl std::future::Future<Output = Result<T, RebornBuildError>>,
) -> Result<T, RebornBuildError> {
    let selected = storage
        .snapshot()
        .map_err(invalid)?
        .validate()
        .map_err(invalid)?;
    tokio::time::timeout(selected.timeout, operation)
        .await
        .map_err(|_| deadline_error(context, selected))?
}

fn deadline_error(
    context: Option<(&'static str, Uuid)>,
    selected: brassclaw_resources::PgResourceStoreSettings,
) -> RebornBuildError {
    let reason = "original observation I/O deadline elapsed; outcome unresolved";
    match context {
        Some((stage, identity)) => invalid(format!(
            "{reason}: {}",
            serde_json::json!({
                "operation":stage,"identity":identity,"settings_revision":selected.revision,
                "timeout_millis":selected.timeout.as_millis().to_string(),
            })
        )),
        None => invalid(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadline_context_contains_only_closed_identity_and_selected_policy() {
        let identity = Uuid::from_u128(1);
        let selected = brassclaw_resources::PgResourceStoreSettings {
            revision: 7,
            timeout: std::time::Duration::from_millis(2000),
        };
        let RebornBuildError::InvalidConfig { reason } =
            deadline_error(Some(("checker_complete_original", identity)), selected)
        else {
            panic!("classified deadline required")
        };
        let (_, context) = reason.split_once(": ").unwrap();
        let value: serde_json::Value = serde_json::from_str(context).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"operation":"checker_complete_original","identity":identity,
            "settings_revision":7,"timeout_millis":"2000"})
        );
        let RebornBuildError::InvalidConfig { reason } = deadline_error(None, selected) else {
            panic!("classified deadline required")
        };
        assert_eq!(
            reason,
            "original observation I/O deadline elapsed; outcome unresolved"
        );
    }

    #[cfg(all(unix, feature = "postgres", feature = "skills-db"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_held_intent_commit_reports_actual_deadline_context_and_keeps_original() {
        use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
        use brassclaw_product_workflow::MontyVmSettingsStore;
        use brassclaw_resources::PgResourceStoreSettingsSource;
        use secrecy::ExposeSecret;
        use std::{sync::Arc, time::Duration};

        let rig = crate::runtime::test_pg::pg_rig().await;
        let settings = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            rig.pool.clone(),
            "default",
            "default",
        )
        .get("default", "default")
        .await
        .unwrap();
        let policy =
            Arc::new(crate::resource_store_settings::AcknowledgedResourceStoreSettings::new());
        policy
            .prepare_bootstrap(
                settings.revision,
                settings.execution_limits.resource_store_timeout_millis,
            )
            .unwrap();
        let selected = policy.snapshot().unwrap();
        let wire = AckCutTransport::start(
            rig.url.expose_secret(),
            vec![AckCutRule::next_commit(1).hold()],
        )
        .await
        .unwrap();
        let pool = Arc::new(
            brassclaw_pg::pool::build_pool_with_max_size(wire.connection_string(), 1).unwrap(),
        );
        let pid = pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT pg_backend_pid()", &[])
            .await
            .unwrap()
            .get::<_, i32>(0);
        let mut ticket = wire.arm_hold(1, pid).unwrap();
        let id = Uuid::new_v4();
        // An actual immutable *unqualified* metadata row, not a native result,
        // component approval or a simulated database acknowledgement.
        let bytes = crate::bootstrap_evidence::system_record(
            id,
            "behavior",
            false,
            &"00".repeat(32),
            &[],
            serde_json::json!({"format":"deadline-context-fixture/1"}),
        );
        let producer = {
            let pool = pool.clone();
            let policy = policy.clone();
            let bytes = bytes.clone();
            tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
                retain_new_intent(&pool, policy.as_ref(), id, &bytes).await
            }))
        };
        let held = tokio::time::timeout(Duration::from_secs(60), ticket.wait_held())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(held.backend_pid, pid);
        let inspector = rig.pool.get().await.unwrap();
        assert_eq!(
            inspector
                .query_one(
                    "SELECT count(*) FROM pg_stat_activity WHERE pid=$1",
                    &[&pid]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1,
            "the real backend remains connected while its COMMIT ACK is held"
        );
        assert_eq!(
            inspector
                .query_one(
                    "SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_id=$1",
                    &[&id]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1,
            "the genuine commit is visible before its withheld acknowledgement"
        );
        drop(inspector);
        let error = tokio::time::timeout(Duration::from_secs(60), producer)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        let RebornBuildError::InvalidConfig { reason } = error else {
            panic!("actual store deadline required")
        };
        let (_, context) = reason
            .split_once(": ")
            .expect("the real timeout must identify its bounded operation");
        let context: serde_json::Value = serde_json::from_str(context).unwrap();
        assert_eq!(
            context,
            serde_json::json!({"operation":"evidence_intent_write","identity":id,
            "settings_revision":selected.revision,"timeout_millis":selected.timeout.as_millis().to_string()})
        );
        tokio::time::timeout(Duration::from_secs(60), ticket.wait_closed())
            .await
            .unwrap()
            .unwrap();
        assert!(
            ticket.release().await.is_err(),
            "the abandoned ACK must never be delivered as success"
        );
        assert_eq!(
            read_record(&pool, policy.as_ref(), id)
                .await
                .unwrap()
                .as_deref(),
            Some(bytes.as_str())
        );
        let client = pool.get().await.unwrap();
        assert_ne!(
            client
                .query_one("SELECT pg_backend_pid()", &[])
                .await
                .unwrap()
                .get::<_, i32>(0),
            pid,
            "pool1 must replace the uncertain transaction connection"
        );
        drop(client);
        pool.close();
        drop(pool);
        wire.shutdown().await.unwrap();
    }
}
