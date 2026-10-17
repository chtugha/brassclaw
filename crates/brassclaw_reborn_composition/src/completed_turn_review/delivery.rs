use super::{
    Package,
    backend::{Backend, PinnedModel},
    ports::ReviewPorts,
};
use crate::{global_monty_owner::GlobalOwnerCheck, monty_kernel::MontyKernelSnapshot};
use brassclaw_host_api::ThreadId;
use brassclaw_llm::SwappableLlmProvider;
use brassclaw_monty_host::service::{ServiceClient, TaskInput};
use brassclaw_pg::PgPool;
use brassclaw_reborn::model_gateway::{LlmModelProfilePolicy, LlmProviderModelGateway};
use brassclaw_threads::{SessionThreadService, ThreadScope};
use brassclaw_turns::{
    TurnActor, TurnId, TurnRunId, TurnScope,
    run_profile::{
        LoopModelBudgetAccountant, LoopRunContext, ModelProfileId, NoOpBudgetAccountant,
        ResolvedRunProfile,
    },
};
use std::sync::{Arc, Mutex, OnceLock, atomic::AtomicBool};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
/// Configuration transport only. The actual provider/model/prefix and isolated
/// accounting owner are captured once before reserving an internal admission.
pub(crate) struct ReviewSource {
    pub preparation: brassclaw_monty_host::utility::UtilityLimits,
    pub provider: Option<Arc<SwappableLlmProvider>>,
    pub accountant: Option<Arc<dyn LoopModelBudgetAccountant>>,
    pub profile: ResolvedRunProfile,
}
struct RetainedReview {
    ports: Arc<ReviewPorts>,
    control: Option<brassclaw_monty_host::service::TaskControl>,
    _credit: brassclaw_monty_host::service::RetainedAttemptCredit,
    attempt: Uuid,
}
type Retentions = Arc<Mutex<Vec<RetainedReview>>>;
// Shutdown cannot destroy unresolved native handles/answers. Recovery remains
// supervised and never re-admits these reservations.
static QUARANTINE: OnceLock<Mutex<Vec<Retentions>>> = OnceLock::new();
type OwnerRegistry = Mutex<Vec<std::sync::Weak<Mutex<Vec<RetainedReview>>>>>;
static OWNERS: OnceLock<OwnerRegistry> = OnceLock::new();
pub(crate) struct ReviewOwner {
    stop: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
    retained: Retentions,
    pool: Arc<PgPool>,
}
impl ReviewOwner {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn start(
        pool: Arc<PgPool>,
        kernel: Arc<dyn MontyKernelSnapshot>,
        service: ServiceClient,
        ownership: GlobalOwnerCheck,
        threads: Arc<dyn SessionThreadService>,
        scope: ThreadScope,
        source: ReviewSource,
    ) -> Result<Self, crate::RebornBuildError> {
        let worker = brassclaw_monty_host::process::installed_worker()
            .map_err(|e| super::invalid(e.to_string()))?;
        let package = Package::boot(pool.clone(), &worker, source.preparation).await?;
        let stop = CancellationToken::new();
        let cancelled = stop.clone();
        let owner = Uuid::new_v4();
        let retained: Retentions = Arc::new(Mutex::new(Vec::new()));
        OWNERS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .map_err(|_| super::invalid("review retention registry unavailable"))?
            .push(Arc::downgrade(&retained));
        let retained_worker = retained.clone();
        let retained_pool = pool.clone();
        let handle = tokio::spawn(async move {
            // Polling is journal delivery, never VM work polling or an agent
            // loop. Python alone selects and executes Recipe steps.
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut last_error = None;
            loop {
                tokio::select! {_=cancelled.cancelled()=>break,_=tick.tick()=>{}}
                let result = match reconcile_retained(&pool, &retained_worker).await {
                    Err(reason) => Err(reason),
                    Ok(()) => {
                        deliver(
                            &pool,
                            &kernel,
                            &service,
                            &ownership,
                            &threads,
                            &scope,
                            &source,
                            &package,
                            owner,
                            &cancelled,
                            &retained_worker,
                        )
                        .await
                    }
                };
                if let Err(reason) = result {
                    if last_error == Some(reason) {
                        continue;
                    }
                    last_error = Some(reason);
                    tracing::warn!(
                        reason,
                        "post-turn review delivery retained for reconciliation"
                    );
                } else {
                    last_error = None;
                }
            }
        });
        Ok(Self {
            stop,
            handle,
            retained,
            pool: retained_pool,
        })
    }
    pub(crate) fn request_shutdown(&self) {
        self.stop.cancel();
    }
    pub(crate) async fn join(self) -> Result<(), crate::RebornRuntimeError> {
        let failed = self.handle.await.is_err();
        let reconciliation_failed = reconcile_retained(&self.pool, &self.retained)
            .await
            .is_err();
        if failed || reconciliation_failed || self.retained.lock().map_or(true, |v| !v.is_empty()) {
            QUARANTINE
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("quarantine mutex")
                .push(self.retained);
            return Err(crate::RebornRuntimeError::InvalidArgument {
                reason: "post-turn review requires supervised reconciliation".into(),
            });
        }
        Ok(())
    }
}
#[allow(clippy::too_many_arguments)]
async fn deliver(
    pool: &Arc<PgPool>,
    kernel: &Arc<dyn MontyKernelSnapshot>,
    service: &ServiceClient,
    ownership: &GlobalOwnerCheck,
    threads: &Arc<dyn SessionThreadService>,
    scope: &ThreadScope,
    source: &ReviewSource,
    package: &Arc<Package>,
    owner: Uuid,
    stop: &CancellationToken,
    retained: &Retentions,
) -> Result<(), &'static str> {
    ownership
        .check()
        .await
        .map_err(|_| "ownership_unavailable")?;
    let Some(provider) = &source.provider else {
        return Ok(());
    };
    let selected = provider.retained_selection();
    let provider = selected.provider;
    let model = selected.model;
    if model == "unconfigured" || model.trim().is_empty() {
        return Ok(());
    }
    let credit = service
        .try_retain_attempt()
        .map_err(|_| "review_retention_capacity")?;
    let client = pool.get().await.map_err(|_| "journal_unavailable")?;
    let row = client
        .query_opt(
            "SELECT e.run_id,e.event_checksum FROM brassclaw_monty_review_events e
        LEFT JOIN brassclaw_monty_review_work w ON w.source_run_id=e.run_id
        WHERE w.source_run_id IS NULL AND e.event_bytes::jsonb #>> '{scope,tenant_id}'=$1
        AND e.event_bytes::jsonb #>> '{scope,agent_id}'=$2 ORDER BY e.recorded_at,e.run_id LIMIT 1",
            &[&scope.tenant_id.as_str(), &scope.agent_id.as_str()],
        )
        .await
        .map_err(|_| "journal_unavailable")?;
    let Some(row) = row else {
        return Ok(());
    };
    let prefix = crate::db_config::list_config_keys(pool, scope.tenant_id.as_str())
        .await
        .map_err(|_| "prefix_unavailable")?
        .into_iter()
        .find(|(key, _)| key == "interceptor.sempai_base_prompt")
        .map(|(_, value)| value);
    let Some(prefix) = prefix.filter(|p| !p.trim().is_empty()) else {
        return Ok(());
    };
    let source_id: Uuid = row.get(0);
    let event_checksum: String = row.get(1);
    let attempt = Uuid::new_v4();
    let thread = ThreadId::new(format!("review:{attempt}")).map_err(|_| "context_invalid")?;
    let turn_scope = TurnScope::new_with_owner(
        scope.tenant_id.clone(),
        Some(scope.agent_id.clone()),
        scope.project_id.clone(),
        thread,
        scope.owner_user_id.clone(),
    );
    let actor = scope
        .owner_user_id
        .clone()
        .ok_or("review_actor_unavailable")?;
    let mut context = LoopRunContext::new(
        turn_scope,
        TurnId::new(),
        TurnRunId::from_uuid(attempt),
        source.profile.clone(),
    )
    .with_actor(TurnActor::new(actor))
    .with_trusted_internal_turn();
    context.resolved_run_profile.model_profile_id =
        ModelProfileId::new("sempai_model").map_err(|_| "context_invalid")?;
    let accountant = source
        .accountant
        .clone()
        .unwrap_or_else(|| Arc::new(NoOpBudgetAccountant));
    let accountant = accountant
        .isolated_work_accountant()
        .ok_or("isolated_accounting_unavailable")?;
    let gateway = Arc::new(LlmProviderModelGateway::with_provider_identity(
        selected.provider_id,
        provider,
        LlmModelProfilePolicy::new().allow_model_profile(
            context.resolved_run_profile.model_profile_id.clone(),
            Some(model.clone()),
        ),
    ));
    let backend = Arc::new(Backend {
        pool: pool.clone(),
        source: source_id,
        attempt,
        owner,
        ownership: ownership.clone(),
        closed: AtomicBool::new(false),
        observed_model: tokio::sync::Mutex::new(None),
        model_work: tokio::sync::Mutex::new(Default::default()),
        threads: threads.clone(),
        scope: scope.clone(),
        context,
        model: PinnedModel {
            name: model.clone(),
            gateway,
            prefix: prefix.clone(),
            accountant,
        },
        selection: package.selection.clone(),
        instructions: package.instructions.clone(),
    });
    // Registration/structural checks precede reservation. No model/effect runs.
    let ports = ReviewPorts::new(backend.clone(), package.clone(), kernel.as_ref())
        .await
        .map_err(|_| "primitive_registration_failed")?;
    ownership
        .check()
        .await
        .map_err(|_| "ownership_unavailable")?;
    retained
        .lock()
        .map_err(|_| "retention_unavailable")?
        .push(RetainedReview {
            ports: ports.clone(),
            control: None,
            _credit: credit,
            attempt,
        });
    let inserted=client.execute("INSERT INTO brassclaw_monty_review_work(source_run_id,attempt_id,owner_id,event_checksum,selection_bytes,prefix_bytes,model_identity)
        VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(source_run_id) DO NOTHING",
        &[&source_id,&attempt,&owner,&event_checksum,&package.selection,&prefix,&model]).await.map_err(|_|"reservation_unresolved")?;
    if inserted != 1 {
        retained
            .lock()
            .map_err(|_| "retention_unavailable")?
            .retain(|r| r.attempt != attempt);
        return Ok(());
    }
    drop(client);
    let input = TaskInput {
        entry: brassclaw_monty_host::service::TaskEntry::InternalRecipe {
            name: "completed-turn-review".into(),
        },
        conversation_id: backend.context.thread_id.to_string(),
        message_id: format!("review-event:{source_id}"),
        turn_id: backend.context.turn_id.to_string(),
        run_id: attempt.to_string(),
        user_input: source_id.to_string(),
        history: vec![],
    };
    let ticket = tokio::select! {
        _=stop.cancelled()=>{ports.retain_failure().await.map_err(|_|"reservation_unresolved")?;return Ok(());}
        result=service.submit_when_available(input,ports.clone())=>result.map_err(|_|"admission_unresolved")?,
    };
    retained
        .lock()
        .map_err(|_| "retention_unavailable")?
        .iter_mut()
        .find(|r| r.attempt == attempt)
        .ok_or("retention_unavailable")?
        .control = Some(ticket.control());
    let result = tokio::select! {
        result=ticket.wait()=>result,
        _=stop.cancelled()=>{ticket.control().cancel();ticket.wait().await}
    };
    match result {
        Ok(receipt) => {
            if !matches!(
                receipt.outcome,
                brassclaw_monty_host::service::TaskOutcome::InternalCompleted { .. }
            ) {
                ports
                    .retain_failure()
                    .await
                    .map_err(|_| "review_settlement_unresolved")?;
            }
            // Retain actual root/accounting settlement independently of an
            // earlier acknowledgement; service failure cannot imply zero usage.
            let evidence = super::reconciliation::settlement(&receipt);
            let changed = pool.get().await.map_err(|_|"review_settlement_unresolved")?.execute(
                "INSERT INTO brassclaw_monty_review_settlements(attempt_id,settlement_bytes) VALUES($1,$2) ON CONFLICT DO NOTHING",
                &[&attempt,&evidence]).await.map_err(|_|"review_settlement_unresolved")?;
            if changed != 1 {
                let existing: String = pool.get().await.map_err(|_| "review_settlement_unresolved")?
                    .query_one("SELECT settlement_bytes FROM brassclaw_monty_review_settlements WHERE attempt_id=$1", &[&attempt])
                    .await.map_err(|_| "review_settlement_unresolved")?.get(0);
                if existing != evidence {
                    return Err("review_settlement_unresolved");
                }
            }
            reconcile_retained(pool, retained).await?;
            Ok(())
        }
        Err(_) => {
            ports
                .retain_failure()
                .await
                .map_err(|_| "review_settlement_unresolved")?;
            Err("service_settlement_unresolved")
        }
    }
}

/// Native recovery maintenance. The delivery owner calls this only between
/// admissions; no recipe sequencing or effect dispatch occurs here.
async fn reconcile_retained(pool: &Arc<PgPool>, retained: &Retentions) -> Result<(), &'static str> {
    let entries: Vec<_> = retained
        .lock()
        .map_err(|_| "retention_unavailable")?
        .iter()
        .filter_map(|r| {
            r.control
                .as_ref()
                .and_then(|control| match control.receipt() {
                    Some(Ok(receipt)) => Some((r.attempt, r.ports.clone(), receipt)),
                    _ => None,
                })
        })
        .collect();
    for (attempt, ports, receipt) in entries {
        if super::reconciliation::reconcile(pool, &ports, &receipt).await? {
            retained
                .lock()
                .map_err(|_| "retention_unavailable")?
                .retain(|r| r.attempt != attempt);
        }
    }
    Ok(())
}

pub(crate) async fn reconcile_attempt(
    pool: &Arc<PgPool>,
    attempt: Uuid,
) -> Result<bool, &'static str> {
    let owners: Vec<_> = OWNERS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .map_err(|_| "retention_unavailable")?
        .iter()
        .filter_map(std::sync::Weak::upgrade)
        .collect();
    for owner in owners {
        let native = owner
            .lock()
            .map_err(|_| "retention_unavailable")?
            .iter()
            .find(|r| {
                r.attempt == attempt && std::ptr::eq(pool.manager(), r.ports.backend.pool.manager())
            })
            .and_then(|r| {
                r.control
                    .as_ref()
                    .and_then(|control| match control.receipt() {
                        Some(Ok(receipt)) => Some((r.ports.clone(), receipt)),
                        _ => None,
                    })
            });
        if let Some((ports, receipt)) = native {
            if super::reconciliation::reconcile(pool, &ports, &receipt).await? {
                let mut entries = owner.lock().map_err(|_| "retention_unavailable")?;
                let before = entries.len();
                entries.retain(|r| r.attempt != attempt);
                // Concurrent maintenance/operator calls may share the same
                // immutable proof. Only the actual remover reports release.
                return Ok(entries.len() != before);
            }
            return Ok(false);
        }
    }
    Ok(false)
}
