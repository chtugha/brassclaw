//! Shared contained bootstrap admission and transport mechanics. No component
//! selection, expected behavior, approval or activation decisions live here.
use crate::bootstrap_reply_validation::invalid;
use crate::{RebornBuildError, pg_monty_admission::PgMontyAdmission};
use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId, UserId};
use brassclaw_loop_support::{
    RunCancellationFactory, RunStateLoopCancellationPort, ThreadBackedLoopTranscriptPort,
    TurnStateRunCancellationFactory,
};
use brassclaw_monty_host::process::{
    ProcessBoundary, ProcessLimits, ProcessSnapshot, RootBoot, TaskHandle, WorkerCommand,
};
use brassclaw_monty_host::transport_actor::{ActorLimits, TransportClient, TransportOwner};
use brassclaw_pg::PgPool;
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, MessageContent, PgSessionThreadService,
    SessionThreadService, ThreadScope,
};
use brassclaw_turns::{
    AcceptedMessageRef, DefaultTurnCoordinator, IdempotencyKey, PgTurnStateStore,
    ReplyTargetBindingRef, SourceBindingRef, SubmitTurnRequest, TurnActor, TurnCoordinator,
    TurnLeaseToken, TurnRunnerId, TurnScope,
    run_profile::{AgentLoopDriverRunRequest, LoopRunContext, MontyTaskAttempt, MontyTaskHandoff},
    runner::{ClaimRunRequest, CompleteRunRequest, TurnRunTransitionPort},
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
use uuid::Uuid;
#[path = "bootstrap_reply_host.rs"]
mod host;

pub(super) const NAMESPACE: &str = "bootstrap-validation";

pub(super) async fn start(
    worker: &Path,
    boot: RootBoot,
    process: ProcessLimits,
    actor: ActorLimits,
    input: Value,
) -> Result<(TransportOwner, TransportClient, ProcessSnapshot, TaskHandle), RebornBuildError> {
    let (owner, ready) = TransportOwner::start(worker, boot, process, actor)
        .await
        .map_err(invalid)?;
    let transport = owner.client();
    let key = ready
        .work_waits
        .first()
        .ok_or_else(|| invalid("bootstrap work wait missing"))?
        .1;
    let mut root = transport
        .try_submit(WorkerCommand::Admit { key, task: input })
        .map_err(invalid)?
        .wait()
        .await
        .map_err(invalid)?
        .outcome
        .map_err(invalid)?;
    let task = root
        .admitted_task
        .ok_or_else(|| invalid("bootstrap task not admitted"))?;
    while let Some(ProcessBoundary::ControlYield { key }) = root.boundary {
        root = transport
            .try_submit(WorkerCommand::ResumeControl { key })
            .map_err(invalid)?
            .wait()
            .await
            .map_err(invalid)?
            .outcome
            .map_err(invalid)?;
    }
    if !matches!(root.boundary, Some(ProcessBoundary::HostCall { ref name, .. }) if name == "validation_hold")
    {
        return Err(invalid("bootstrap root did not reach contained hold"));
    }
    Ok((owner, transport, root, task))
}

pub(super) async fn close(
    owner: &mut TransportOwner,
    admitted: &Admitted,
) -> Result<(), RebornBuildError> {
    owner.request_termination();
    let exit = owner.join().await.map_err(invalid)?;
    if exit.exit_status.is_none() || exit.containment_error.is_some() || exit.reap_error.is_some() {
        return Err(invalid("bootstrap worker containment is unresolved"));
    }
    settle_closed(admitted).await
}

/// Close a contained private validation claim after its worker was reaped.
pub(super) async fn settle_closed(admitted: &Admitted) -> Result<(), RebornBuildError> {
    admitted
        .admission
        .settle(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
        .await
        .map_err(invalid)?;
    admitted
        .state
        .complete_run(CompleteRunRequest {
            run_id: admitted.attempt.run_id,
            runner_id: admitted.attempt.runner_id,
            lease_token: admitted.attempt.lease_token,
        })
        .await
        .map_err(invalid)?;
    Ok(())
}

/// Contained source observers may acknowledge a late cancellation only after
/// the actual model-free worker, host calls and transport have quiesced. Other
/// contained callers retain their existing cleanup contract.
pub(super) async fn settle_quiescent_closed(
    pool: &PgPool,
    admitted: &Admitted,
    proof: &crate::bootstrap_recipe_flow::ModelFreeQuiescence,
) -> Result<brassclaw_turns::TurnStatus, RebornBuildError> {
    use brassclaw_turns::{
        TurnSpawnTreeStateStore, TurnStatus, runner::CancelRunCompletionRequest,
    };
    proof.require_run(admitted.attempt.run_id)?;
    let original = admitted
        .state
        .get_run_record(&admitted.host.run_context().scope, admitted.attempt.run_id)
        .await
        .map_err(invalid)?
        .ok_or_else(|| invalid("contained original Turn missing before settlement"))?;
    if original.native_auth_wait.is_some() {
        return Err(invalid("contained native custody is unresolved"));
    }
    #[cfg(test)]
    crate::validator_inventory_review::installation_test_before_close(
        admitted.attempt.run_id.as_uuid(),
    )?;
    if let Err(ack) = admitted
        .admission
        .settle(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
        .await
    {
        // Only re-read the exact immutable original. Resolving its lost ACK
        // permits the first Turn close, never another effect or reservation.
        require_source_admission_closed(pool, admitted)
            .await
            .map_err(|readback| {
                invalid(format!(
                    "original Admission close failed: {ack}; original readback failed: {readback}"
                ))
            })?;
    }
    #[cfg(test)]
    crate::validator_inventory_review::installation_test_before_turn_close(
        admitted.attempt.run_id.as_uuid(),
    )?;
    let complete_error = match admitted
        .state
        .complete_run(CompleteRunRequest {
            run_id: admitted.attempt.run_id,
            runner_id: admitted.attempt.runner_id,
            lease_token: admitted.attempt.lease_token,
        })
        .await
    {
        Ok(state) => return Ok(state.status),
        Err(error) => error,
    };
    let record=admitted.state.get_run_record(&admitted.host.run_context().scope,admitted.attempt.run_id).await.map_err(|readback|
        invalid(format!("original Turn complete failed: {complete_error}; original Turn readback failed: {readback}")))?
        .ok_or_else(||invalid(format!("original Turn complete failed: {complete_error}; original Turn missing during settlement")))?;
    if record.status != TurnStatus::CancelRequested || record.native_auth_wait.is_some() {
        return Err(invalid(complete_error));
    }
    #[cfg(test)]
    crate::validator_inventory_review::installation_test_before_cancel(
        admitted.attempt.run_id.as_uuid(),
    )?;
    let state = admitted
        .state
        .cancel_run(CancelRunCompletionRequest {
            run_id: admitted.attempt.run_id,
            runner_id: admitted.attempt.runner_id,
            lease_token: admitted.attempt.lease_token,
        })
        .await
        .map_err(invalid)?;
    // Cancelled is successful technical cleanup, never qualification success.
    Ok(state.status)
}

async fn require_source_admission_closed(
    pool: &PgPool,
    admitted: &Admitted,
) -> Result<(), RebornBuildError> {
    use sha2::{Digest, Sha256};
    let mut client = crate::bootstrap_evidence::io::EvidenceClient::acquire(pool).await?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(invalid)?;
    let row=tx.query_opt("SELECT phase,outcome,scope,runner_id,claim_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1",&[&admitted.attempt.run_id.as_uuid()]).await.map_err(invalid)?
        .ok_or_else(||invalid("contained original Admission missing"))?;
    if row.get::<_, String>(0) != "settled"
        || row.get::<_, Option<Value>>(1)
            != Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
        || row.get::<_, Value>(2) != json!(admitted.host.run_context().scope)
        || row.get::<_, Uuid>(3) != admitted.attempt.runner_id.as_uuid()
        || row.get::<_, Vec<u8>>(4)
            != Sha256::digest(serde_json::to_vec(&admitted.attempt.lease_token).map_err(invalid)?)
                .to_vec()
    {
        return Err(invalid("contained original Admission close not confirmed"));
    }
    tx.commit().await.map_err(invalid)?;
    client.recycle_after_commit();
    Ok(())
}

/// Read both original owners without holding a client across Turn lookup.
/// The acknowledged technical Store source bounds this complete logical read.
/// This can resolve a lost ACK but performs no transition or task replay.
pub(super) async fn read_source_close(
    pool: &PgPool,
    admitted: &Admitted,
    proof: Option<&crate::bootstrap_recipe_flow::ModelFreeQuiescence>,
    acknowledgement: Result<brassclaw_turns::TurnStatus, RebornBuildError>,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
) -> Result<brassclaw_turns::TurnRunRecord, RebornBuildError> {
    use brassclaw_turns::{TurnSpawnTreeStateStore, TurnStatus};
    let readback = crate::bootstrap_evidence::io::bounded(storage, async {
        require_source_admission_closed(pool, admitted).await?;
        let record = admitted
            .state
            .get_run_record(&admitted.host.run_context().scope, admitted.attempt.run_id)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("contained original Turn missing after close"))?;
        if record.run_id != admitted.attempt.run_id
            || record.scope != admitted.host.run_context().scope
            || record.claim_count != 1
            || record.runner_id.is_some()
            || record.lease_token.is_some()
            || record.lease_expires_at.is_some()
            || record.failure.is_some()
            || record.native_auth_wait.is_some()
        {
            return Err(invalid(
                "contained original Turn settlement identity/custody incomplete",
            ));
        }
        match record.status {
            TurnStatus::Completed => {}
            TurnStatus::Cancelled => proof
                .ok_or_else(|| invalid("cancelled original lacks model-free quiescence"))?
                .require_run(record.run_id)?,
            other => {
                return Err(invalid(format!(
                    "contained original cleanup incomplete: Turn {other:?}"
                )));
            }
        }
        Ok(record)
    })
    .await;
    match (acknowledgement, readback) {
        (Ok(status), Ok(record)) if status == record.status => Ok(record),
        (Ok(status), Ok(record)) => Err(invalid(format!(
            "cleanup acknowledged {status:?}; original Turn instead {:?}",
            record.status
        ))),
        (Err(ack), Ok(record)) => {
            tracing::info!(acknowledgement_error=%ack,status=?record.status,"contained close ACK resolved by exact original Admission and Turn readback");
            Ok(record)
        }
        (Ok(status), Err(readback)) => Err(invalid(format!(
            "cleanup acknowledged {status:?}; original verification failed: {readback}"
        ))),
        (Err(ack), Err(readback)) => Err(invalid(format!(
            "original cleanup failed: {ack}; original close readback failed: {readback}"
        ))),
    }
}

pub(crate) struct Admitted {
    pub(crate) host: Arc<MontyTaskHost>,
    pub(super) input: Value,
    pub(super) admission: Arc<PgMontyAdmission>,
    pub(super) state: Arc<PgTurnStateStore>,
    pub(super) attempt: MontyTaskAttempt,
}
pub(super) async fn admit(
    pool: Arc<PgPool>,
    identity: &str,
    answer: &str,
    host_factory: Option<&dyn brassclaw_reborn::turn_runner::HostFactory>,
) -> Result<Admitted, RebornBuildError> {
    let mode = host_factory
        .map(AdmissionMode::ModelContained)
        .unwrap_or(AdmissionMode::Contained);
    prepare_admission(pool, identity, answer, mode).await
}

/// Reserve a trusted internal validator task admission.
///
/// Sets `trusted_internal_turn = true` so that `capture_internal` is reached
/// in `InstalledMontyCatalogue`. Must be called after `retain_validator_selection`
/// has succeeded for `submission_id`.
///
/// The returned `Admitted::host` must be passed to
/// `GlobalMontyDriver::drive_admitted`. After `drive_admitted` returns, no
/// explicit admission cleanup is required — the factory's `settle` /
/// `settle_unsubmitted` path owns the lifecycle.
pub(crate) async fn admit_validator_handoff(
    pool: Arc<PgPool>,
    submission_id: uuid::Uuid,
    reference: &str,
) -> Result<Admitted, RebornBuildError> {
    let identity = format!("validator-{}", submission_id.as_simple());
    prepare_admission(pool, &identity, reference, AdmissionMode::ValidatorInternal).await
}

/// Reserve a real installation-review parent without starting VM execution.
/// The owner must retain its captured catalogue, live claim and scheduled ports
/// before dispatch. This supplies no approval, activation or child authority by
/// itself; the native child owner still verifies every original reference.
#[cfg(test)]
pub(super) async fn reserve_child_review_parent(
    pool: Arc<PgPool>,
    identity: &str,
    reference: &str,
) -> Result<Admitted, RebornBuildError> {
    prepare_admission(pool, identity, reference, AdmissionMode::RetainedParent).await
}

#[derive(Clone, Copy)]
enum AdmissionMode<'a> {
    Contained,
    /// The closed installation owner supplies the existing host factory. It
    /// must expose the real model/accounting/custody ports for this subject;
    /// unavailable support fails instead of falling back to the reply host.
    ModelContained(&'a dyn brassclaw_reborn::turn_runner::HostFactory),
    /// Trusted internal validator task.  Sets `trusted_internal_turn = true`
    /// so that `capture_internal` is reached in `InstalledMontyCatalogue`.
    /// Must be called after `retain_validator_selection` has succeeded.
    /// Does NOT call `check_and_start`; internal tasks bypass model selection
    /// via the trusted flag.
    /// Constructed by `ValidatorTaskDriver::submit` via `admit_validator_handoff`.
    ValidatorInternal,
    #[cfg(test)]
    RetainedParent,
}

async fn prepare_admission(
    pool: Arc<PgPool>,
    identity: &str,
    answer: &str,
    mode: AdmissionMode<'_>,
) -> Result<Admitted, RebornBuildError> {
    let tenant = TenantId::new(NAMESPACE).map_err(invalid)?;
    let agent = AgentId::new(identity).map_err(invalid)?;
    let actor = UserId::new("installation-owner").map_err(invalid)?;
    let (project, owner) = match mode {
        AdmissionMode::Contained => (Some(ProjectId::new("contained").map_err(invalid)?), None),
        AdmissionMode::ModelContained(_) => (None, Some(actor.clone())),
        AdmissionMode::ValidatorInternal => (None, Some(actor.clone())),
        #[cfg(test)]
        AdmissionMode::RetainedParent => (None, Some(actor.clone())),
    };
    let source_binding = match mode {
        AdmissionMode::Contained => "bootstrap-internal",
        AdmissionMode::ModelContained(_) => {
            brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING
        }
        AdmissionMode::ValidatorInternal => {
            brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING
        }
        #[cfg(test)]
        AdmissionMode::RetainedParent => {
            brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING
        }
    };
    let conversation = ThreadId::new(format!("{identity}-{}", Uuid::new_v4())).map_err(invalid)?;
    let scope = ThreadScope {
        tenant_id: tenant.clone(),
        agent_id: agent.clone(),
        project_id: project.clone(),
        owner_user_id: owner,
    };
    let threads = Arc::new(PgSessionThreadService::new(pool.clone(), tenant.as_str()));
    threads
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: Some(conversation.clone()),
            created_by_actor_id: actor.to_string(),
            title: Some("Internal bootstrap validation".into()),
            metadata_json: Some(json!({"bootstrap_validation":true}).to_string()),
        })
        .await
        .map_err(invalid)?;
    let accepted = threads
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: scope.clone(),
            thread_id: conversation.clone(),
            actor_id: actor.to_string(),
            source_binding_id: Some(source_binding.into()),
            reply_target_binding_id: Some(source_binding.into()),
            external_event_id: None,
            content: MessageContent::text(answer),
        })
        .await
        .map_err(invalid)?;
    let turn_scope = match mode {
        AdmissionMode::Contained => {
            TurnScope::new(tenant.clone(), Some(agent), project, conversation.clone())
        }
        AdmissionMode::ModelContained(_) | AdmissionMode::ValidatorInternal => {
            TurnScope::new_with_owner(
                tenant.clone(),
                Some(agent),
                project,
                conversation.clone(),
                scope.owner_user_id.clone(),
            )
        }
        #[cfg(test)]
        AdmissionMode::RetainedParent => TurnScope::new_with_owner(
            tenant.clone(),
            Some(agent),
            project,
            conversation.clone(),
            scope.owner_user_id.clone(),
        ),
    };
    let state = Arc::new(PgTurnStateStore::new(pool.clone(), tenant.as_str()));
    let coordinator = DefaultTurnCoordinator::new(state.clone());
    let coordinator = match mode {
        AdmissionMode::Contained => coordinator,
        AdmissionMode::ModelContained(_) | AdmissionMode::ValidatorInternal => coordinator
            .with_run_profile_resolver(Arc::new(
                brassclaw_reborn::planned_driver_factory::default_planned_run_profile_resolver()
                    .map_err(invalid)?,
            )),
        #[cfg(test)]
        AdmissionMode::RetainedParent => coordinator.with_run_profile_resolver(Arc::new(
            brassclaw_reborn::planned_driver_factory::default_planned_run_profile_resolver()
                .map_err(invalid)?,
        )),
    };
    coordinator
        .submit_turn(SubmitTurnRequest {
            scope: turn_scope.clone(),
            actor: TurnActor::new(actor),
            accepted_message_ref: AcceptedMessageRef::new(format!("msg:{}", accepted.message_id))
                .map_err(invalid)?,
            source_binding_ref: SourceBindingRef::new(source_binding).map_err(invalid)?,
            reply_target_binding_ref: ReplyTargetBindingRef::new(source_binding)
                .map_err(invalid)?,
            requested_run_profile: None,
            idempotency_key: IdempotencyKey::new(Uuid::new_v4().to_string()).map_err(invalid)?,
            received_at: chrono::Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        })
        .await
        .map_err(invalid)?;
    let claimed = state
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(turn_scope.clone()),
        })
        .await
        .map_err(invalid)?
        .ok_or_else(|| invalid("bootstrap claim missing"))?;
    threads
        .mark_message_submitted(
            &scope,
            &conversation,
            accepted.message_id,
            claimed.state.turn_id.to_string(),
            claimed.state.run_id.to_string(),
        )
        .await
        .map_err(invalid)?;
    let context = LoopRunContext::new(
        turn_scope,
        claimed.state.turn_id,
        claimed.state.run_id,
        claimed.resolved_run_profile.clone(),
    )
    .with_actor(
        claimed
            .state
            .actor
            .clone()
            .ok_or_else(|| invalid("bootstrap actor missing"))?,
    )
    .with_accepted_message_ref(claimed.state.accepted_message_ref.clone());
    let context = match mode {
        AdmissionMode::Contained | AdmissionMode::ModelContained(_) => context,
        AdmissionMode::ValidatorInternal => context.with_trusted_internal_turn(),
        #[cfg(test)]
        AdmissionMode::RetainedParent => context.with_trusted_internal_turn(),
    };
    let attempt = MontyTaskAttempt {
        run_id: claimed.state.run_id,
        runner_id: claimed.runner_id,
        lease_token: claimed.lease_token,
    };
    let original_source = claimed.state.source_binding_ref.clone();
    let handoff = async {
        let host: Arc<dyn brassclaw_turns::run_profile::AgentLoopDriverHost + Send + Sync> =
            match mode {
                AdmissionMode::ModelContained(factory) => {
                    Arc::from(factory.create_host(&claimed).await.map_err(invalid)?)
                }
                _ => {
                    let cancellation_owner =
                        Arc::new(TurnStateRunCancellationFactory::new(state.clone()));
                    let cancellation = RunStateLoopCancellationPort::new(
                        cancellation_owner
                            .handle_for_run(&context.scope, context.run_id)
                            .await
                            .map_err(invalid)?,
                    );
                    Arc::new(host::BootstrapReplyHost {
                        transcript: ThreadBackedLoopTranscriptPort::new(
                            threads,
                            scope,
                            context.clone(),
                        ),
                        cancellation,
                        _cancellation_owner: cancellation_owner,
                    })
                }
            };
        let request = AgentLoopDriverRunRequest {
            turn_id: context.turn_id,
            run_id: context.run_id,
            resolved_run_profile: context.resolved_run_profile.clone(),
        };
        match mode {
            AdmissionMode::Contained => MontyTaskHandoff::new(request, attempt, host),
            AdmissionMode::ModelContained(_) | AdmissionMode::ValidatorInternal => {
                MontyTaskHandoff::new_claimed(request, Arc::new(claimed), host)
            }
            #[cfg(test)]
            AdmissionMode::RetainedParent => {
                MontyTaskHandoff::new_claimed(request, Arc::new(claimed), host)
            }
        }
        .map_err(invalid)
    }
    .await;
    let handoff = match handoff {
        Ok(handoff) => handoff,
        Err(error) => {
            // Port construction is effect-free and no admission was reserved.
            // Close this original claimed run rather than leaking Running work
            // or replacing it with a restricted host. A failed close remains
            // visible; it must not be reported as successful cleanup.
            state
                .fail_run(brassclaw_turns::runner::FailRunRequest {
                    run_id: attempt.run_id,
                    runner_id: attempt.runner_id,
                    lease_token: attempt.lease_token,
                    failure: brassclaw_turns::SanitizedFailure::new(
                        "bootstrap_host_preparation_failed",
                    )
                    .map_err(invalid)?,
                })
                .await
                .map_err(invalid)?;
            return Err(error);
        }
    };
    let host = Arc::new(MontyTaskHost::new(handoff));
    // Construct and identity-check the real ports before retaining an admission
    // address. A failed model-host preparation cannot lose a reserved address or
    // silently substitute the restricted reply-only host. Factories execute no
    // task effects; the first operation still requires the durable admission.
    let admission = Arc::new(
        PgMontyAdmission::prepare(pool, &context, attempt, Some(&original_source))
            .map_err(invalid)?,
    );
    // ValidatorInternal delegates persist_reservation to OwnedGlobalTaskFactory::build
    // (via GlobalMontyDriver::drive_admitted → ports.build) to avoid a double-insert
    // on brassclaw_monty_task_admissions. All other modes persist here.
    if !matches!(mode, AdmissionMode::ValidatorInternal) {
        #[cfg(test)]
        if identity == "installation-source-observer" && matches!(mode, AdmissionMode::Contained) {
            crate::validator_inventory_review::installation_test_before_reservation(
                attempt.run_id.as_uuid(),
                answer,
            )?;
        }
        admission.persist_reservation().await.map_err(invalid)?;
    }
    if matches!(mode, AdmissionMode::Contained) {
        admission.check_and_start().await.map_err(invalid)?;
    }
    // ValidatorInternal: trusted_internal_turn is already set on the context.
    // check_and_start is not called; internal tasks bypass model selection.
    // ModelContained: check_and_start is also not called (factory supplies ports).
    Ok(Admitted {
        host,
        admission,
        state,
        attempt,
        input: json!({"conversation_id":conversation,"message_id":accepted.message_id,"turn_id":context.turn_id,"run_id":context.run_id,"user_input":answer,"history":[]}),
    })
}
