//! In-process delivery of the existing opaque native completion. These slots
//! neither admit nor claim work, own no timer, and cannot restore proof from SQL.
use super::*;
use brassclaw_turns::TurnRunId;
use std::sync::Weak;
use tokio::sync::watch;

type ResultValue = Result<DrivenMontyTask, AgentLoopDriverError>;
pub(super) struct PrivateDelivery {
    result: watch::Sender<Option<ResultValue>>,
}
pub(super) type PrivateDeliveries = Mutex<HashMap<TurnRunId, Arc<PrivateDelivery>>>;

pub(crate) struct PrivateCompletionWait {
    run: TurnRunId,
    deliveries: Weak<PrivateDeliveries>,
    original: Weak<PrivateDelivery>,
    result: watch::Receiver<Option<ResultValue>>,
    service: ServiceClient,
}
impl PrivateCompletionWait {
    pub(crate) fn has_result(&self) -> bool {
        self.result.borrow().is_some()
    }
    pub(crate) async fn wait(mut self) -> ResultValue {
        loop {
            let result = self.result.borrow().clone();
            if let Some(result) = result {
                return result;
            }
            tokio::select! {
                changed=self.result.changed()=>{
                    changed.map_err(|_|failed("private_completion_delivery_closed"))?;
                }
                ()=self.service.closed()=>return Err(failed("private_completion_service_closed")),
            }
        }
    }
}
impl Drop for PrivateCompletionWait {
    fn drop(&mut self) {
        if let Some(deliveries) = self.deliveries.upgrade()
            && let Ok(mut slots) = deliveries.lock()
            && let Some(slot) = slots.get(&self.run)
            && self
                .original
                .upgrade()
                .is_some_and(|original| Arc::ptr_eq(slot, &original))
            && slot.result.receiver_count() <= 1
        {
            slots.remove(&self.run);
        }
        // Dropping a waiter never cancels work or manufactures a native ACK.
    }
}
impl GlobalMontyDriver {
    pub(crate) fn private_completion_waiter(
        &self,
        run: TurnRunId,
    ) -> Result<PrivateCompletionWait, AgentLoopDriverError> {
        let mut slots = self
            .private_deliveries
            .lock()
            .map_err(|_| failed("private_completion_delivery_failed"))?;
        let slot = slots.entry(run).or_insert_with(|| {
            Arc::new(PrivateDelivery {
                result: watch::channel(None).0,
            })
        });
        Ok(PrivateCompletionWait {
            run,
            deliveries: Arc::downgrade(&self.private_deliveries),
            original: Arc::downgrade(slot),
            result: slot.result.subscribe(),
            service: self.service.clone(),
        })
    }
    pub(super) async fn drive_original_private(
        &self,
        claimed: Arc<brassclaw_turns::private_work::ClaimedPrivateWork>,
    ) -> Result<(), AgentLoopDriverError> {
        let delivery = self
            .private_deliveries
            .lock()
            .map_err(|_| failed("private_completion_delivery_failed"))?
            .get(&claimed.request().run_id())
            .cloned();
        let result = async {
            let host = self.ports.private_host(&claimed)?;
            self.drive_host(host).await
        }
        .await;
        if let Some(delivery) = delivery {
            delivery.result.send_replace(Some(result.clone()));
        }
        result.map(|_| ())
    }
}
