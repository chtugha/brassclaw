//! One captured PostgreSQL policy for original authoring and its readback.
//! Native preparation is not a PostgreSQL exchange. No retry or replay lives here.
#[cfg(feature = "skills-db")]
use brassclaw_product_workflow::RecipeStoreError;
use brassclaw_resources::PgResourceStoreSettings;
use std::future::Future;
#[cfg(all(test, feature = "skills-db"))]
use std::sync::{Arc, Mutex};

pub(crate) struct SubmissionIo {
    settings: PgResourceStoreSettings,
    #[cfg(all(test, feature = "skills-db"))]
    observation: Option<Arc<Mutex<Observation>>>,
}
impl SubmissionIo {
    #[cfg(feature = "skills-db")]
    pub(crate) fn capture(settings: PgResourceStoreSettings) -> Result<Self, RecipeStoreError> {
        Ok(Self {
            settings: settings.validate().map_err(|_| {
                RecipeStoreError::Unavailable(
                    "original review storage unavailable; retain original request for readback"
                        .into(),
                )
            })?,
            #[cfg(all(test, feature = "skills-db"))]
            observation: OBSERVATION.try_with(Arc::clone).ok(),
        })
    }
    // Same canonical compatibility policy as the existing resource stores when
    // no running catalogue/settings source is attached. No new numeric limit.
    #[cfg(feature = "skills-db")]
    pub(crate) fn compatibility() -> Result<Self, RecipeStoreError> {
        Self::capture(PgResourceStoreSettings {
            revision: 1,
            timeout: std::time::Duration::from_millis(
                brassclaw_host_api::MontyExecutionLimits::default().resource_store_timeout_millis,
            ),
        })
    }
    pub(crate) async fn run<T, E>(
        &self,
        phase: &'static str,
        operation: impl Future<Output = Result<T, E>>,
    ) -> Result<Result<T, E>, ()> {
        self.note(phase, "started");
        match tokio::time::timeout(self.settings.timeout, operation).await {
            Ok(result) => {
                self.note(phase, if result.is_ok() { "ok" } else { "error" });
                Ok(result)
            }
            Err(_) => {
                self.note(phase, "deadline");
                Err(())
            }
        }
    }
    pub(crate) fn note(&self, phase: &'static str, status: &'static str) {
        #[cfg(not(all(test, feature = "skills-db")))]
        let _ = (phase, status);
        #[cfg(all(test, feature = "skills-db"))]
        if let Some(observation) = &self.observation
            && let Ok(mut observed) = observation.try_lock()
        {
            observed.phase = phase;
            observed.status = status;
            observed.revision = self.settings.revision;
            if matches!(status, "error" | "deadline") && observed.first_failure.is_none() {
                observed.first_failure = Some((phase, status));
            }
            if status == "started" && matches!(phase, "author_commit" | "queue_author_commit") {
                observed.author_commit_started = true;
            }
            if status == "commit_acknowledgement_lost" {
                observed.commit_acknowledgement_lost = true;
            }
        }
    }
}

#[cfg(all(test, feature = "skills-db"))]
#[derive(Default)]
struct Observation {
    phase: &'static str,
    status: &'static str,
    revision: u64,
    first_failure: Option<(&'static str, &'static str)>,
    author_commit_started: bool,
    commit_acknowledgement_lost: bool,
}
#[cfg(all(test, feature = "skills-db"))]
tokio::task_local! { static OBSERVATION: Arc<Mutex<Observation>>; }
#[cfg(all(test, feature = "skills-db"))]
pub(crate) async fn observe<F: Future>(operation: F) -> (F::Output, serde_json::Value) {
    let observation = Arc::new(Mutex::new(Observation::default()));
    let result = OBSERVATION.scope(observation.clone(), operation).await;
    let facts = observation.try_lock().map(|observed| serde_json::json!({
        "phase":observed.phase,"status":observed.status,"policy_revision":observed.revision,
        "first_failure":observed.first_failure,"author_commit_started":observed.author_commit_started,"commit_acknowledgement_lost":observed.commit_acknowledgement_lost
    })).unwrap_or_else(|_|serde_json::json!({"observation":"unavailable"}));
    (result, facts)
}
