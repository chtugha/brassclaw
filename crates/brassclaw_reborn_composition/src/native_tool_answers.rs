//! Actual received native answers and their original immutable journal handles.
//! Reconciliation acknowledges evidence only; it cannot dispatch or resume code.
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError as Error;
use brassclaw_monty_host::process::PortAnswer;
use brassclaw_reborn::monty_model_capability::{
    KOHAI_COMPLETE_CAPABILITY_ID, RecipeModelResponseStore,
};

use crate::pg_monty_admission::PgMontyInvocation;

#[derive(Default)]
pub(super) struct NativeToolAnswers {
    entries: Mutex<BTreeMap<String, Arc<NativeToolAnswer>>>,
}

pub(super) struct NativeToolAnswer {
    record: PgMontyInvocation,
    capability: String,
    answer: Mutex<Option<PortAnswer>>,
    acknowledged: AtomicBool,
}
impl NativeToolAnswer {
    pub(super) fn receive(&self, answer: PortAnswer) -> Result<PortAnswer, Error> {
        let mut retained = self.answer.lock().map_err(|_| Error)?;
        if retained.is_some() || self.acknowledged.load(Ordering::Acquire) {
            return Err(Error);
        }
        *retained = Some(answer.clone());
        Ok(answer)
    }

    pub(super) async fn acknowledge(&self) -> Result<(), Error> {
        // JsonValue clones share immutable payload custody; no recursive Value
        // copy/serialization or lock is held across database I/O.
        let answer = {
            let retained = self.answer.lock().map_err(|_| Error)?;
            if self.acknowledged.load(Ordering::Acquire) {
                return Ok(());
            }
            retained.as_ref().cloned().ok_or(Error)?
        };
        self.record
            .record_answer(&answer)
            .await
            .map_err(|_| Error)?;
        let mut retained = self.answer.lock().map_err(|_| Error)?;
        // Keep identity and acknowledgement, but release redundant host payload
        // only after the original exact journal write has been confirmed. The
        // dispatcher still owns the answer it will deliver to Monty. Concurrent
        // acknowledgements of this same immutable record remain idempotent.
        retained.take();
        self.acknowledged.store(true, Ordering::Release);
        Ok(())
    }
}
impl NativeToolAnswers {
    pub(super) fn retain(
        &self,
        step: &str,
        capability: &str,
        record: PgMontyInvocation,
    ) -> Result<Arc<NativeToolAnswer>, Error> {
        let mut entries = self.entries.lock().map_err(|_| Error)?;
        if entries.contains_key(step) {
            return Err(Error);
        }
        let received = Arc::new(NativeToolAnswer {
            record,
            capability: capability.to_owned(),
            answer: Mutex::new(None),
            acknowledged: AtomicBool::new(false),
        });
        #[cfg(test)]
        observations::retain(&received);
        entries.insert(step.to_owned(), received.clone());
        Ok(received)
    }

    pub(super) fn has_pending(&self) -> bool {
        self.entries
            .lock()
            .map(|entries| {
                entries
                    .values()
                    .any(|answer| !answer.acknowledged.load(Ordering::Acquire))
            })
            .unwrap_or(true)
    }

    pub(super) async fn reconcile(
        &self,
        model: Option<&dyn RecipeModelResponseStore>,
    ) -> Result<(), Error> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| Error)?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        if let Some(model) = model {
            for entry in &entries {
                if entry.capability == KOHAI_COMPLETE_CAPABILITY_ID
                    && model.has_uncommitted_results()
                {
                    // This existing model-owner API also only acknowledges
                    // actual wire/provider/response bytes. Never call a model.
                    model
                        .reconcile_received_response(entry.record.invocation_id())
                        .await
                        .map_err(|_| Error)?;
                }
            }
            if model.has_uncommitted_results() {
                return Err(Error);
            }
        }
        for entry in entries {
            if !entry.acknowledged.load(Ordering::Acquire) {
                entry.acknowledge().await?;
            }
        }
        Ok(())
    }
}

// Read-only weak witnesses for the real native COMMIT regression. They do not
// retain payloads, supply results or change the production acknowledgement path.
#[cfg(test)]
pub(super) fn retains_payload_for_test(invocation: uuid::Uuid) -> Result<bool, Error> {
    observations::retains_payload(invocation)
}

#[cfg(test)]
mod observations {
    use super::*;
    use std::sync::{OnceLock, Weak};

    type Witnesses = BTreeMap<uuid::Uuid, Vec<Weak<NativeToolAnswer>>>;
    static ANSWERS: OnceLock<Mutex<Witnesses>> = OnceLock::new();

    pub(super) fn retain(answer: &Arc<NativeToolAnswer>) {
        let mut answers = ANSWERS
            .get_or_init(Mutex::default)
            .lock()
            .expect("native answer witnesses available");
        answers.retain(|_, entries| {
            entries.retain(|answer| answer.strong_count() > 0);
            !entries.is_empty()
        });
        let invocation = answer.record.invocation_id().as_uuid();
        answers
            .entry(invocation)
            .or_default()
            .push(Arc::downgrade(answer));
    }

    pub(super) fn retains_payload(invocation: uuid::Uuid) -> Result<bool, Error> {
        let answers = ANSWERS
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|_| Error)?
            .get(&invocation)
            .into_iter()
            .flatten()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        for answer in answers {
            if answer.answer.lock().map_err(|_| Error)?.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
