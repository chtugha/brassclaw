//! In-process delivery of the existing opaque native completion. These slots
//! neither admit nor claim work, own no timer, and cannot restore proof from SQL.
use super::*;
use brassclaw_turns::TurnRunId;
#[cfg(test)]
use std::sync::Weak;
use tokio::sync::watch;

type ResultValue = Result<DrivenMontyTask, AgentLoopDriverError>;
struct PrivateDelivery {
    result: watch::Sender<Option<ResultValue>>,
}
pub(super) struct DeliverySlot {
    channel: Arc<PrivateDelivery>,
    producer: Option<MontyTaskAttempt>,
}
pub(super) type PrivateDeliveries = Mutex<HashMap<TurnRunId, DeliverySlot>>;

#[cfg(test)]
pub(crate) struct PrivateCompletionWait {
    run: TurnRunId,
    deliveries: Weak<PrivateDeliveries>,
    original: Weak<PrivateDelivery>,
    result: watch::Receiver<Option<ResultValue>>,
}
#[cfg(test)]
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
            // Closing admission precedes containment and the producer's actual
            // result. Keep this original subscription through that interval.
            // ProducerDelivery/Worker stop publish failure on interruption;
            // missing evidence stays pending under the caller's test watchdog.
            self.result
                .changed()
                .await
                .map_err(|_| failed("private_completion_delivery_closed"))?;
        }
    }
}
#[cfg(test)]
impl Drop for PrivateCompletionWait {
    fn drop(&mut self) {
        if let Some(deliveries) = self.deliveries.upgrade()
            && let Ok(mut slots) = deliveries.lock()
            && let Some(slot) = slots.get(&self.run)
            && self
                .original
                .upgrade()
                .is_some_and(|original| Arc::ptr_eq(&slot.channel, &original))
            && slot.producer.is_none()
            && slot.channel.result.receiver_count() <= 1
        {
            slots.remove(&self.run);
        }
        // A running producer keeps its slot even when all current waiters leave.
        // Dropping a waiter never cancels work or manufactures a native ACK.
    }
}

/// Disposal reports loss of the producer future, never successful settlement.
/// The original slot stays pinned until the Worker's addressed stop/fence path
/// ends; a late subscriber must observe the same unresolved result.
struct ProducerDelivery {
    deliveries: Arc<PrivateDeliveries>,
    run: TurnRunId,
    channel: Arc<PrivateDelivery>,
    published: bool,
}
impl ProducerDelivery {
    fn publish(&mut self, result: ResultValue) {
        let succeeded = result.is_ok();
        self.channel.result.send_replace(Some(result));
        self.published = true;
        if succeeded
            && let Ok(mut slots) = self.deliveries.lock()
            && let Some(slot) = slots.get_mut(&self.run)
            && Arc::ptr_eq(&slot.channel, &self.channel)
        {
            // drive_host success includes the actual native settlement.
            slot.producer = None;
            if slot.channel.result.receiver_count() == 0 {
                slots.remove(&self.run);
            }
        }
    }
}
impl Drop for ProducerDelivery {
    fn drop(&mut self) {
        if !self.published {
            self.channel
                .result
                .send_replace(Some(Err(failed("private_completion_producer_interrupted"))));
        }
    }
}
fn new_slot() -> DeliverySlot {
    DeliverySlot {
        channel: Arc::new(PrivateDelivery {
            result: watch::channel(None).0,
        }),
        producer: None,
    }
}
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PauseStage {
    BeforeDrive,
    BeforeSubmit,
    BeforePublish,
}
#[cfg(test)]
pub(crate) struct PublishPause {
    pub(crate) reached: Notify,
    pub(crate) release: Notify,
    pub(crate) consumer_waiting: Notify,
    stage: PauseStage,
}
#[cfg(test)]
impl PublishPause {
    pub(super) async fn wait(&self) {
        self.reached.notify_one();
        self.release.notified().await;
    }
}
impl GlobalMontyDriver {
    #[cfg(test)]
    pub(crate) fn pause_private_publish(&self, run: TurnRunId) -> Arc<PublishPause> {
        self.pause_private_at(run, PauseStage::BeforePublish)
    }
    #[cfg(test)]
    pub(crate) fn pause_private_start(&self, run: TurnRunId) -> Arc<PublishPause> {
        self.pause_private_at(run, PauseStage::BeforeDrive)
    }
    #[cfg(test)]
    pub(crate) fn pause_private_before_submit(&self, run: TurnRunId) -> Arc<PublishPause> {
        self.pause_private_at(run, PauseStage::BeforeSubmit)
    }
    #[cfg(test)]
    pub(super) fn private_submit_pause(&self, run: TurnRunId) -> Option<Arc<PublishPause>> {
        self.private_publish_pauses
            .lock()
            .unwrap()
            .get(&run)
            .filter(|pause| pause.stage == PauseStage::BeforeSubmit)
            .cloned()
    }
    #[cfg(test)]
    fn pause_private_at(&self, run: TurnRunId, stage: PauseStage) -> Arc<PublishPause> {
        let pause = Arc::new(PublishPause {
            reached: Notify::new(),
            release: Notify::new(),
            consumer_waiting: Notify::new(),
            stage,
        });
        assert!(
            self.private_publish_pauses
                .lock()
                .unwrap()
                .insert(run, pause.clone())
                .is_none()
        );
        pause
    }
    #[cfg(test)]
    async fn private_test_pause(&self, run: TurnRunId, stage: PauseStage) {
        let pause = self
            .private_publish_pauses
            .lock()
            .unwrap()
            .get(&run)
            .filter(|pause| pause.stage == stage)
            .cloned();
        if let Some(pause) = pause {
            pause.reached.notify_one();
            pause.release.notified().await;
            self.private_publish_pauses.lock().unwrap().remove(&run);
        }
    }
    #[cfg(test)]
    pub(crate) fn private_consumer_waiting(&self, run: TurnRunId) {
        if let Some(pause) = self.private_publish_pauses.lock().unwrap().get(&run) {
            pause.consumer_waiting.notify_one();
        }
    }
    #[cfg(test)]
    pub(crate) async fn prepare_private_validator(
        &self,
        submission_id: uuid::Uuid,
    ) -> Result<brassclaw_turns::private_work::PrivateWorkState, crate::RebornBuildError> {
        self.ports.prepare_private_validator(submission_id).await
    }
    /// Hold the same slot lock used to register the actual producer. Once it
    /// starts, its delivery lifetime covers the interval after factory release
    /// and before publish; retained-host absence cannot reject that live wait.
    #[cfg(test)]
    pub(crate) fn private_completion_live(
        &self,
        run: TurnRunId,
    ) -> Result<bool, AgentLoopDriverError> {
        let slots = self
            .private_deliveries
            .lock()
            .map_err(|_| failed("private_completion_delivery_failed"))?;
        let Some(slot) = slots.get(&run) else {
            return Ok(false);
        };
        if slot.producer.is_some() || slot.channel.result.borrow().is_some() {
            return Ok(true);
        }
        // A real claim can exist before drive_private is first polled. Holding
        // slots excludes producer registration until this retention read ends.
        self.ports.private_attempt_retained(run)
    }
    #[cfg(test)]
    pub(crate) fn private_completion_waiter(
        &self,
        run: TurnRunId,
    ) -> Result<PrivateCompletionWait, AgentLoopDriverError> {
        let mut slots = self
            .private_deliveries
            .lock()
            .map_err(|_| failed("private_completion_delivery_failed"))?;
        let slot = slots.entry(run).or_insert_with(new_slot);
        Ok(PrivateCompletionWait {
            run,
            deliveries: Arc::downgrade(&self.private_deliveries),
            original: Arc::downgrade(&slot.channel),
            result: slot.channel.result.subscribe(),
        })
    }
    pub(super) fn private_producer_stopped(&self, attempt: MontyTaskAttempt) {
        if let Ok(mut slots) = self.private_deliveries.lock()
            && let Some(slot) = slots.get_mut(&attempt.run_id)
            && (slot.producer.is_none() || slot.producer == Some(attempt))
        {
            // Also covers a stop before the driver's future was first polled.
            let missing = slot.channel.result.borrow().is_none();
            if missing {
                slot.channel
                    .result
                    .send_replace(Some(Err(failed("private_completion_producer_stopped"))));
            }
            slot.producer = None;
            if slot.channel.result.receiver_count() == 0 {
                slots.remove(&attempt.run_id);
            }
        }
    }
    pub(super) async fn drive_original_private(
        &self,
        claimed: Arc<brassclaw_turns::private_work::ClaimedPrivateWork>,
    ) -> Result<(), AgentLoopDriverError> {
        let run = claimed.request().run_id();
        let channel = {
            let mut slots = self
                .private_deliveries
                .lock()
                .map_err(|_| failed("private_completion_delivery_failed"))?;
            let slot = slots.entry(run).or_insert_with(new_slot);
            if slot.producer.is_some() || slot.channel.result.borrow().is_some() {
                return Err(failed("private_completion_producer_already_started"));
            }
            slot.producer = Some(claimed.attempt());
            slot.channel.clone()
        };
        let mut delivery = ProducerDelivery {
            deliveries: self.private_deliveries.clone(),
            run,
            channel,
            published: false,
        };
        #[cfg(test)]
        self.private_test_pause(run, PauseStage::BeforeDrive).await;
        let result = async {
            let host = self.ports.private_host(&claimed)?;
            self.drive_host(host).await
        }
        .await;
        #[cfg(test)]
        self.private_test_pause(run, PauseStage::BeforePublish)
            .await;
        #[cfg(test)]
        self.private_publish_pauses.lock().unwrap().remove(&run);
        delivery.publish(result.clone());
        result.map(|_| ())
    }
}
