//! Existing worker lifecycle participant. Original discovery is read-only;
//! only genuine delivered observations reach their existing review owner.
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{ReviewWorkStatus, ReviewWorkView};
use brassclaw_turns::{
    private_work::{PrivateExecutionContext, PrivateWorkRequest, PrivateWorkStatus},
    run_profile::{AgentLoopDriverError, PrivateWorkReconciliationPort},
};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio_postgres::Transaction;
use uuid::Uuid;

pub(crate) struct ReviewWorkReconciler {
    pool: Arc<PgPool>,
    catalogue: Arc<crate::installed_monty_catalogue::InstalledMontyCatalogue>,
    context: PrivateExecutionContext,
    cursor: tokio::sync::Mutex<Option<(DateTime<Utc>, Uuid)>>,
    case_cursor: tokio::sync::Mutex<Option<(DateTime<Utc>, Uuid)>>,
}
fn invalid() -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: "review_work_original_invalid".into(),
    }
}
fn unavailable() -> AgentLoopDriverError {
    AgentLoopDriverError::Unavailable {
        reason: "original review reconciliation unavailable".into(),
    }
}
impl ReviewWorkReconciler {
    pub(crate) fn new(
        pool: Arc<PgPool>,
        catalogue: Arc<crate::installed_monty_catalogue::InstalledMontyCatalogue>,
        context: PrivateExecutionContext,
    ) -> Self {
        Self {
            pool,
            catalogue,
            context,
            cursor: tokio::sync::Mutex::new(None),
            case_cursor: tokio::sync::Mutex::new(None),
        }
    }
    /// Existing lifecycle participant discovers case originals independently of
    /// review confirmations. It never creates Work or replays a native case.
    async fn reconcile_cases(
        &self,
        capacity: i64,
        deadline: std::time::Duration,
    ) -> Result<(), AgentLoopDriverError> {
        let mut cursor = self.case_cursor.lock().await;
        let (after_time, after_id) = cursor
            .as_ref()
            .map(|(time, id)| (Some(*time), Some(*id)))
            .unwrap_or((None, None));
        let rows=tokio::time::timeout(deadline,async {
            let client=self.pool.get().await.map_err(|_|unavailable())?;
            client.query("SELECT c.recorded_at,c.case_id,c.execution_context FROM reborn_original_pure_cases c
                WHERE ($1::timestamptz IS NULL OR (c.recorded_at,c.case_id)>($1,$2::uuid))
                AND EXISTS(SELECT 1 FROM brassclaw_monty_private_deliveries d WHERE d.work_id=c.case_id)
                AND NOT EXISTS(SELECT 1 FROM reborn_original_pure_case_settlements s WHERE s.case_id=c.case_id)
                ORDER BY c.recorded_at,c.case_id LIMIT $3",&[&after_time,&after_id,&capacity]).await.map_err(|_|unavailable())
        }).await.map_err(|_|unavailable())??;
        if rows.is_empty() {
            *cursor = None;
            return Ok(());
        }
        let mut first_error = None;
        for row in rows {
            let id: Uuid = row.get(1);
            *cursor = Some((row.get(0), id));
            let result = match serde_json::from_value::<PrivateExecutionContext>(row.get(2)) {
                Err(_) => Err(invalid()),
                Ok(context) => {
                    if context.tenant_id != self.context.tenant_id
                        || context.agent_id != self.context.agent_id
                        || context.project_id != self.context.project_id
                    {
                        continue;
                    }
                    crate::pure_case_settlement::settle(
                        &self.pool,
                        id,
                        &context,
                        &self.catalogue.pure_source_review,
                        deadline,
                    )
                    .await
                }
            };
            match result {
                Ok(Some(original)) => {
                    tracing::debug!(case_id=%original.original().id(),settlement_checksum=%hex::encode(original.checksum()),
                        observation_bytes=original.observation_bytes().len(),"original Pure case delivery acknowledged");
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(case_id=%id,%error,"original Pure case delivery remains unresolved");
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
    #[cfg(test)]
    pub(crate) async fn original_cursor(&self) -> Option<Uuid> {
        self.cursor.lock().await.as_ref().map(|(_, id)| *id)
    }
}
#[async_trait::async_trait]
impl PrivateWorkReconciliationPort for ReviewWorkReconciler {
    fn notify_originals(&self) -> Result<(), AgentLoopDriverError> {
        self.catalogue
            .private_work_wake
            .get()
            .ok_or_else(unavailable)?
            .wake();
        Ok(())
    }
    async fn reconcile_originals(&self) -> Result<(), AgentLoopDriverError> {
        // This is the acknowledged outstanding DB-ownership work capacity,
        // not a quota on reviews or a newly invented polling/backoff setting.
        let policy = self
            .catalogue
            .preparation_policy()
            .await
            .map_err(|_| unavailable())?;
        let capacity = i64::from(policy.max_pending_ownership_checks);
        let deadline = std::time::Duration::from_millis(policy.resource_store_timeout_millis);
        let case_error = self.reconcile_cases(capacity, deadline).await.err();
        let mut cursor = self.cursor.lock().await;
        let (after_time, after_id) = cursor
            .as_ref()
            .map(|(time, id)| (Some(*time), Some(*id)))
            .unwrap_or((None, None));
        let rows = tokio::time::timeout(deadline, async {
            let client = self.pool.get().await.map_err(|_| unavailable())?;
            client.query("SELECT r.requested_at,r.submission_id FROM reborn_component_review_requests r
                WHERE ($1::timestamptz IS NULL OR (r.requested_at,r.submission_id)>($1,$2::uuid))
                AND NOT EXISTS(SELECT 1 FROM reborn_component_review_work_confirmations c WHERE c.submission_id=r.submission_id)
                ORDER BY r.requested_at,r.submission_id LIMIT $3",&[&after_time,&after_id,&capacity]).await.map_err(|_|unavailable())
        }).await.map_err(|_|unavailable())??;
        if rows.is_empty() {
            *cursor = None;
            return case_error.map_or(Ok(()), Err);
        }
        let mut first_error = case_error;
        for row in rows {
            let id: Uuid = row.get(1);
            *cursor = Some((row.get(0), id));
            let result=tokio::time::timeout(deadline,async {
                let mut client=crate::bootstrap_evidence::io::acquire(&self.pool).await.map_err(|_|unavailable())?;
                let tx=client.build_transaction()
                    .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                    .read_only(true).start().await.map_err(|_|unavailable())?;
                let view=inspect_with_source(&tx,id,Some(&self.catalogue.pure_source_review)).await?;
                let mut confirmation_context=None;
                if view.delivery_checksum.is_some() && view.confirmation_checksum.is_none() {
                    let row=tx.query_one("SELECT private_request_bytes FROM brassclaw_turns WHERE validator_submission_id=$1 AND record_kind='private_work'",&[&id]).await.map_err(|_|unavailable())?;
                    let request:PrivateWorkRequest=serde_json::from_str(row.get(0)).map_err(|_|invalid())?;
                    // Preserve the original actor; another instance/context is
                    // visible but cannot be adopted by this worker.
                    let context=request.context();
                    if context.tenant_id==self.context.tenant_id && context.agent_id==self.context.agent_id && context.project_id==self.context.project_id {
                        confirmation_context=Some(context.clone());
                    }
                }
                tracing::debug!(submission_id=%id,status=?view.status,"original review work inspected");
                tx.commit().await.map_err(|_|unavailable())?;
                client.recycle_after_commit();
                drop(client);
                if let Some(context)=confirmation_context {
                    crate::review_work_confirmation::confirm_original_with_source(&self.pool,id,&context,Some(&self.catalogue.pure_source_review)).await?;
                }
                Ok(())
            }).await.unwrap_or_else(|_|Err(unavailable()));
            if let Err(error) = result {
                tracing::warn!(submission_id=%id,%error,"original review remains unresolved");
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

/// Same original projection used by worker discovery and authenticated operator
/// inspection. Missing Work never causes a new selection or preparation.
#[cfg(test)]
pub(crate) async fn inspect(
    tx: &Transaction<'_>,
    id: Uuid,
) -> Result<ReviewWorkView, AgentLoopDriverError> {
    inspect_with_source(tx, id, None).await
}
pub(crate) async fn inspect_with_source(
    tx: &Transaction<'_>,
    id: Uuid,
    source: Option<&crate::validator_inventory_review::QualifiedPureSourceReview>,
) -> Result<ReviewWorkView, AgentLoopDriverError> {
    let row=tx.query_opt("SELECT r.subject_checksum,s.checksum,s.subject_bytes,
        q.submission_id,w.private_request_bytes,w.status,d.checksum,c.checksum,
        EXISTS(SELECT 1 FROM reborn_component_validator_selections v WHERE v.submission_id=r.submission_id)
        FROM reborn_component_review_requests r JOIN reborn_component_review_submissions s USING(submission_id)
        LEFT JOIN reborn_component_review_queue_requests q USING(submission_id)
        LEFT JOIN brassclaw_turns w ON w.validator_submission_id=r.submission_id AND w.record_kind='private_work'
        LEFT JOIN brassclaw_monty_private_deliveries d ON d.work_id=w.work_id
        LEFT JOIN reborn_component_review_work_confirmations c ON c.submission_id=r.submission_id
        WHERE r.submission_id=$1",&[&id]).await.map_err(|_|unavailable())?.ok_or_else(invalid)?;
    let subject: &str = row.get(2);
    if row.get::<_, &str>(0) != row.get::<_, &str>(1)
        || row.get::<_, &str>(0) != hex::encode(Sha256::digest(subject.as_bytes()))
    {
        return Err(invalid());
    }
    let mut view = ReviewWorkView {
        status: ReviewWorkStatus::OriginalPreparationMissing,
        work_id: None,
        run_id: None,
        delivery_checksum: None,
        confirmation_checksum: None,
        selection_preparation_present: row.get(8),
        q1_qualified: false,
    };
    let Some(request_bytes) = row.get::<_, Option<&str>>(4) else {
        let doc: serde_json::Value = serde_json::from_str(subject).map_err(|_| invalid())?;
        view.status = if row.get::<_, Option<Uuid>>(3).is_none() {
            ReviewWorkStatus::LegacyCustodyUnresolved
        } else if doc.get("function_partner").is_some() {
            ReviewWorkStatus::JointSelectionUnsupported
        } else {
            ReviewWorkStatus::OriginalPreparationMissing
        };
        if !crate::validation_queue::ValidationQueueStore::original_queue_is_current(tx, id)
            .await
            .map_err(|_| invalid())?
        {
            view.status = ReviewWorkStatus::OriginalQueueChanged;
        }
        return Ok(view);
    };
    let request: PrivateWorkRequest = serde_json::from_str(request_bytes).map_err(|_| invalid())?;
    if !matches!(request.original(),brassclaw_turns::private_work::PrivateOriginalRef::ValidatorStructural {submission_id,subject_checksum}
        if *submission_id==id && hex::encode(subject_checksum)==row.get::<_,&str>(0))
    {
        return Err(invalid());
    }
    let state = brassclaw_turns::PgTurnStateStore::read_private_work_state_in_transaction(
        tx,
        request.context().tenant_id.as_str(),
        request.work_id(),
    )
    .await
    .map_err(|_| invalid())?
    .ok_or_else(invalid)?;
    view.work_id = Some(request.work_id().to_string());
    view.run_id = Some(request.run_id().as_uuid().to_string());
    view.status = match state.status {
        PrivateWorkStatus::AwaitingPreparation => ReviewWorkStatus::AwaitingPreparation,
        PrivateWorkStatus::Ready => ReviewWorkStatus::Ready,
        PrivateWorkStatus::Claimed => ReviewWorkStatus::Running,
        PrivateWorkStatus::ReconciliationRequired => ReviewWorkStatus::ReconciliationRequired,
        PrivateWorkStatus::Cancelled => ReviewWorkStatus::Cancelled,
        PrivateWorkStatus::Settled => ReviewWorkStatus::NativeTerminalUnacknowledged,
    };
    if row.get::<_, Option<Vec<u8>>>(6).is_some() {
        let delivery = crate::pg_monty_admission::PgMontyAdmission::read_private_delivery(
            tx,
            request.work_id(),
            request.context(),
        )
        .await?
        .ok_or_else(invalid)?;
        view.delivery_checksum = Some(hex::encode(delivery.checksum()));
        view.status = ReviewWorkStatus::FullQ1QualificationMissing;
    }
    if let Some(checksum) = row.get::<_, Option<Vec<u8>>>(7) {
        if view.delivery_checksum.is_none() {
            return Err(invalid());
        }
        crate::review_work_confirmation::read_with_source(tx, id, request.context(), source)
            .await?
            .ok_or_else(invalid)?;
        view.confirmation_checksum = Some(hex::encode(checksum));
        view.status = ReviewWorkStatus::StructuralObservationsConfirmed;
    } else if !crate::validation_queue::ValidationQueueStore::original_queue_is_current(tx, id)
        .await
        .map_err(|_| invalid())?
    {
        view.status = ReviewWorkStatus::OriginalQueueChanged;
    }
    Ok(view)
}
