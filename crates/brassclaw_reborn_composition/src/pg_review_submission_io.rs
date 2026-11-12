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
    projection: Option<Projection>,
}
#[cfg(all(test, feature = "skills-db"))]
tokio::task_local! { static OBSERVATION: Arc<Mutex<Observation>>; }
#[cfg(all(test, feature = "skills-db"))]
fn facts(observation: &Arc<Mutex<Observation>>) -> serde_json::Value {
    observation.try_lock().map(|observed| serde_json::json!({
        "phase":observed.phase,"status":observed.status,"policy_revision":observed.revision,
        "first_failure":observed.first_failure,"author_commit_started":observed.author_commit_started,"commit_acknowledgement_lost":observed.commit_acknowledgement_lost,
        "projection":observed.projection.as_ref().map(Projection::facts),
    })).unwrap_or_else(|_|serde_json::json!({"observation":"unavailable"}))
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) async fn observe<F: Future>(operation: F) -> (F::Output, serde_json::Value) {
    let observation = Arc::new(Mutex::new(Observation::default()));
    let result = OBSERVATION.scope(observation.clone(), operation).await;
    (result, facts(&observation))
}

// Passive, bounded test state in the existing observer owner. A handle survives
// outer watchdog cancellation, so facts do not depend on reissuing a read.
#[cfg(all(test, feature = "skills-db"))]
#[derive(Clone, Default)]
pub(crate) struct ReadObservation(Arc<Mutex<Observation>>);
#[cfg(all(test, feature = "skills-db"))]
impl ReadObservation {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) async fn observe<F: Future>(&self, operation: F) -> F::Output {
        OBSERVATION.scope(self.0.clone(), operation).await
    }
    pub(crate) fn facts(&self) -> serde_json::Value {
        facts(&self.0)
    }
}
#[cfg(all(test, feature = "skills-db"))]
struct Projection {
    started: std::time::Instant,
    budget_started: Option<std::time::Instant>,
    ended: Option<std::time::Instant>,
    revision: Option<u64>,
    timeout_millis: Option<u128>,
    phase: &'static str,
    index: Option<usize>,
    first_error: Option<ProjectionError>,
    deadline_selected: bool,
    result: Option<&'static str>,
}
#[cfg(all(test, feature = "skills-db"))]
#[derive(serde::Serialize)]
struct ProjectionError {
    phase: &'static str,
    index: Option<usize>,
    kind: &'static str,
    class: Option<&'static str>,
    sqlstate: Option<String>,
}
#[cfg(all(test, feature = "skills-db"))]
impl Projection {
    fn facts(&self) -> serde_json::Value {
        let end = self.ended.unwrap_or_else(std::time::Instant::now);
        serde_json::json!({
            "phase":self.phase,"index":self.index,"policy_revision":self.revision,
            "timeout_millis":self.timeout_millis,"elapsed_millis":end.duration_since(self.started).as_millis(),
            "io_elapsed_millis":self.budget_started.map(|start|end.duration_since(start).as_millis()),
            "deadline_selected":self.deadline_selected,"result":self.result,"first_error":self.first_error,
        })
    }
}
#[cfg(all(test, feature = "skills-db"))]
fn with_projection(note: impl FnOnce(&mut Projection)) {
    let _ = OBSERVATION.try_with(|observation| {
        if let Ok(mut observed) = observation.try_lock()
            && let Some(projection) = &mut observed.projection
        {
            note(projection);
        }
    });
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_start() {
    let _ = OBSERVATION.try_with(|observation| {
        if let Ok(mut observed) = observation.try_lock() {
            observed.projection = Some(Projection {
                started: std::time::Instant::now(),
                budget_started: None,
                ended: None,
                revision: None,
                timeout_millis: None,
                phase: "capture_policy",
                index: None,
                first_error: None,
                deadline_selected: false,
                result: None,
            });
        }
    });
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_policy(settings: PgResourceStoreSettings) {
    with_projection(|p| {
        p.revision = Some(settings.revision);
        p.timeout_millis = Some(settings.timeout.as_millis());
        p.budget_started = Some(std::time::Instant::now());
    });
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_phase(phase: &'static str, index: Option<usize>) {
    with_projection(|p| {
        p.phase = phase;
        p.index = index;
    });
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_error(
    kind: &'static str,
    class: Option<&'static str>,
    sqlstate: Option<&str>,
) {
    with_projection(|p| {
        if p.first_error.is_none() {
            p.first_error = Some(ProjectionError {
                phase: p.phase,
                index: p.index,
                kind,
                class,
                sqlstate: sqlstate.map(str::to_owned),
            });
        }
    });
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_pool_error(error: &deadpool_postgres::PoolError) {
    use deadpool_postgres::PoolError;
    let (kind, sqlstate) = match error {
        PoolError::Timeout(_) => ("PoolTimeout", None),
        PoolError::Backend(error) => ("PoolBackend", error.code().map(|code| code.code())),
        PoolError::Closed => ("PoolClosed", None),
        PoolError::NoRuntimeSpecified => ("PoolNoRuntimeSpecified", None),
        PoolError::PostCreateHook(_) => ("PoolPostCreateHook", None),
    };
    projection_error(kind, None, sqlstate);
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_driver_error(error: &brassclaw_turns::run_profile::AgentLoopDriverError) {
    use brassclaw_turns::run_profile::AgentLoopDriverError::*;
    let (kind, class) = match error {
        InputAdmissionPending => ("InputAdmissionPending", None),
        InvalidRequest { .. } => ("InvalidRequest", None),
        Unavailable { .. } => ("Unavailable", None),
        Failed { reason_kind } => (
            "Failed",
            match reason_kind.as_str() {
                "review_work_original_conflict" => Some("review_work_original_conflict"),
                "private_delivery_original_invalid" => Some("private_delivery_original_invalid"),
                "private_terminal_original_invalid" => Some("private_terminal_original_invalid"),
                "monty_admission_database_failed" => Some("monty_admission_database_failed"),
                _ => None,
            },
        ),
    };
    projection_error(kind, class, None);
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_build_error(error: &crate::RebornBuildError) {
    use crate::RebornBuildError::*;
    let kind = match error {
        OriginalPureEncoderQualification { .. } => "OriginalPureEncoderQualification",
        OriginalPureCaseQualification { .. } => "OriginalPureCaseQualification",
        InvalidConfig { .. } => "InvalidConfig",
        MissingDatabaseHandle { .. } => "MissingDatabaseHandle",
        MissingProductionTrustPolicy => "MissingProductionTrustPolicy",
        MissingRuntimePolicy => "MissingRuntimePolicy",
        EmptyProductionTrustPolicy => "EmptyProductionTrustPolicy",
        MissingTurnRunWakeNotifier => "MissingTurnRunWakeNotifier",
        MissingSecretMasterKey => "MissingSecretMasterKey",
        PlannedRunProfileResolver { .. } => "PlannedRunProfileResolver",
        ProductionWiring { .. } => "ProductionWiring",
        HostRuntime(..) => "HostRuntime",
        EventStore(..) => "EventStore",
        Secret(..) => "Secret",
        Filesystem(..) => "Filesystem",
        Resource(..) => "Resource",
        RunState(..) => "RunState",
        CapabilityLease(..) => "CapabilityLease",
        Turn(..) => "Turn",
        Mount(..) => "Mount",
        ContentIntegrity(..) => "ContentIntegrity",
    };
    projection_error(kind, None, None);
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_deadline() {
    with_projection(|p| p.deadline_selected = true);
}
#[cfg(all(test, feature = "skills-db"))]
pub(crate) fn projection_finish<T>(result: &Result<T, RecipeStoreError>) {
    let kind = match result {
        Ok(_) => "ok",
        Err(error) => match error {
            RecipeStoreError::Invalid(_) => "Invalid",
            RecipeStoreError::NotFound(_) => "NotFound",
            RecipeStoreError::Conflict(_) => "Conflict",
            RecipeStoreError::Unavailable(_) => "Unavailable",
            RecipeStoreError::Internal(_) => "Internal",
        },
    };
    if result.is_err() {
        projection_error(kind, None, None);
    }
    with_projection(|p| {
        p.ended = Some(std::time::Instant::now());
        p.result = Some(kind);
    });
}
