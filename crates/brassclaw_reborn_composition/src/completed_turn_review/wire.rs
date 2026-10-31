//! Native custody for the existing internal review's one provider operation.
//! No retry, proposal, completion or budget decision is made by this observer.
use super::backend::{Backend, checksum};
use crate::global_monty_owner::GlobalOwnerCheck;
use async_trait::async_trait;
use brassclaw_llm::provider_observation::{ProviderObservationError, ProviderWireObserver};
use brassclaw_pg::PgPool;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

pub(super) struct ReviewWire {
    pub pool: Arc<PgPool>,
    pub attempt: Uuid,
    pub owner: Uuid,
    pub ownership: GlobalOwnerCheck,
    pub closed: Arc<AtomicBool>,
    pub provider_id: String,
    pub model: String,
    // The backend retains this slot through cancellation/persistence failure.
    // Received originals are owned synchronously, never reconstructed or clipped.
    pub held: Arc<Mutex<Option<String>>>,
    rejected: AtomicBool,
    started: AtomicBool,
}
impl ReviewWire {
    pub(super) fn new(backend: &Backend) -> Self {
        Self {
            pool: backend.pool.clone(),
            attempt: backend.attempt,
            owner: backend.owner,
            ownership: backend.ownership.clone(),
            closed: backend.closed.clone(),
            provider_id: backend.model.provider_id.clone(),
            model: backend.model.name.clone(),
            held: backend.wire_original.clone(),
            rejected: AtomicBool::new(false),
            started: AtomicBool::new(false),
        }
    }
    pub(super) fn confirmed_unsubmitted(&self) -> bool {
        self.rejected.load(Ordering::Acquire) && !self.started.load(Ordering::Acquire)
    }
}
#[async_trait]
impl ProviderWireObserver for ReviewWire {
    async fn before_submission(&self) -> Result<(), ProviderObservationError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ProviderObservationError);
        }
        self.ownership
            .check()
            .await
            .map_err(|_| ProviderObservationError)?;
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProviderObservationError)?;
        let valid:bool=client.query_one("SELECT EXISTS(SELECT 1 FROM brassclaw_monty_review_work WHERE attempt_id=$1 AND owner_id=$2 AND phase='model_dispatching' AND model_dispatch_count=1 AND model_binding_bytes IS NOT NULL)",
            &[&self.attempt,&self.owner]).await.map_err(|_| ProviderObservationError)?.get(0);
        if !valid || self.closed.load(Ordering::Acquire) {
            return Err(ProviderObservationError);
        }
        Ok(())
    }
    fn submission_started(&self) {
        self.started.store(true, Ordering::Release);
    }
    fn submission_rejected(&self) {
        self.rejected.store(true, Ordering::Release);
    }
    fn receive_response(&self, status: u16, body: &[u8]) -> Result<(), ProviderObservationError> {
        let bytes = serde_json::to_string(&brassclaw_llm::provider_observation::wire_envelope(
            status,
            body,
            &self.provider_id,
            &self.model,
        ))
        .map_err(|_| ProviderObservationError)?;
        let mut held = self.held.lock().map_err(|_| ProviderObservationError)?;
        if held.is_some() {
            return Err(ProviderObservationError);
        }
        *held = Some(bytes);
        Ok(())
    }
    async fn persist_response(&self) -> Result<(), ProviderObservationError> {
        let bytes = self
            .held
            .lock()
            .map_err(|_| ProviderObservationError)?
            .clone()
            .ok_or(ProviderObservationError)?;
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| ProviderObservationError)?;
        // A lost write acknowledgement is read back at the SAME attempt. Never
        // repeat the request to repair native response evidence.
        let _write=client.execute("UPDATE brassclaw_monty_review_work SET provider_wire_bytes=$3,provider_wire_checksum=$4 WHERE attempt_id=$1 AND owner_id=$2 AND provider_wire_bytes IS NULL",
            &[&self.attempt,&self.owner,&bytes,&checksum(&bytes)]).await;
        let row=client.query_opt("SELECT provider_wire_bytes,provider_wire_checksum FROM brassclaw_monty_review_work WHERE attempt_id=$1 AND owner_id=$2",
            &[&self.attempt,&self.owner]).await.map_err(|_| ProviderObservationError)?.ok_or(ProviderObservationError)?;
        if row.get::<_, Option<String>>(0).as_ref() != Some(&bytes)
            || row.get::<_, Option<String>>(1).as_deref() != Some(checksum(&bytes).as_str())
        {
            return Err(ProviderObservationError);
        }
        Ok(())
    }
}
