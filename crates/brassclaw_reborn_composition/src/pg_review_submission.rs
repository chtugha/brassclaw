//! Operator authoring boundary. Only immutable, unreviewed subjects are written;
//! explicit queue capture may create its pending legacy projection atomically.
//! No evidence record or active catalogue is changed here.
use std::sync::Arc;

use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    ComponentFunctionReviewPartner, ComponentFunctionReviewPartnerReceipt,
    ComponentReviewSubmissionReceipt, ComponentReviewSubmissionView, ComponentRevisionSelection,
    RecipeStoreError, SubmitComponentReviewRequest,
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, RevisionError},
    review_submission_store::{
        FunctionReviewPartnerDraft, PgReviewSubmissionStore, RetainedReviewSubmission,
        ReviewSubmissionDraft, SubmissionStoreError,
    },
    revision_store::RevisionStoreError,
};
use uuid::Uuid;

use crate::pg_review_submission_io::SubmissionIo;
#[cfg(test)]
pub(crate) use crate::pg_review_submission_io::observe as observe_submission_io;

fn revision_parse_error(error: RevisionError, kind: &str) -> RecipeStoreError {
    RecipeStoreError::Invalid(match error {
        RevisionError::UnsupportedDepth => RevisionError::UnsupportedDepth.to_string(),
        _ => format!("invalid immutable {kind} revision"),
    })
}
fn identifier(value: &str) -> Result<Uuid, RecipeStoreError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| RecipeStoreError::Invalid("canonical non-nil UUID required".into()))
}
pub(crate) fn reference(
    value: &ComponentRevisionSelection,
) -> Result<ComponentRevisionRef, RecipeStoreError> {
    if value.version == 0
        || value.version > i64::MAX as u64
        || !matches!(value.class_code, 0..=10 | 12..=23 | 50)
        || value.checksum.len() != 64
        || !value
            .checksum
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(RecipeStoreError::Invalid(
            "exact revision reference required".into(),
        ));
    }
    let mut checksum = [0; 32];
    for (slot, pair) in checksum
        .iter_mut()
        .zip(value.checksum.as_bytes().as_chunks::<2>().0.iter())
    {
        let digit = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
        *slot = digit(pair[0]) * 16 + digit(pair[1]);
    }
    Ok(ComponentRevisionRef {
        uuid: identifier(&value.uuid)?,
        class_code: value.class_code,
        version: value.version,
        checksum,
    })
}
pub(crate) fn selection(value: ComponentRevisionRef) -> ComponentRevisionSelection {
    ComponentRevisionSelection {
        uuid: value.uuid.to_string(),
        class_code: value.class_code,
        version: value.version,
        checksum: value
            .checksum
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}
fn receipt(value: &RetainedReviewSubmission) -> ComponentReviewSubmissionReceipt {
    ComponentReviewSubmissionReceipt {
        preparation_revision: value.preparation.as_ref().map(|p| p.revision()),
        queue_ids: vec![],
        queue_binding_status: brassclaw_product_workflow::ReviewQueueBindingStatus::Standalone,
        submission_id: value.id.to_string(),
        subject_checksum: value.checksum.clone(),
        candidate: selection(value.candidate),
        base: value.base.map(selection),
        dependencies: value.dependencies.iter().copied().map(selection).collect(),
        function_partner: value.function_partner.map(|partner| {
            ComponentFunctionReviewPartnerReceipt {
                reference: selection(partner.reference),
                changed: partner.changed,
                base: partner.base.map(selection),
            }
        }),
        review_status: "unreviewed".into(),
        catalogue_activated: false,
    }
}
async fn original_receipt(
    pool: &Arc<PgPool>,
    retained: &RetainedReviewSubmission,
) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
    let mut result = receipt(retained);
    let client = pool.get().await.map_err(storage_io)?;
    if let Some(row) = client
        .query_opt(
            "SELECT queue_ids FROM reborn_component_review_queue_requests WHERE submission_id=$1",
            &[&retained.id],
        )
        .await
        .map_err(storage_io)?
    {
        result.queue_ids = row
            .get::<_, Vec<Uuid>>(0)
            .into_iter()
            .map(|id| id.to_string())
            .collect();
    }
    if !result.queue_ids.is_empty() {
        result.queue_binding_status = brassclaw_product_workflow::ReviewQueueBindingStatus::Bound;
    }
    Ok(result)
}
fn storage(value: SubmissionStoreError) -> RecipeStoreError {
    match value {
        SubmissionStoreError::Conflict
        | SubmissionStoreError::Revisions(RevisionStoreError::Conflict) => {
            RecipeStoreError::Conflict(value.to_string())
        }
        SubmissionStoreError::Database(_)
        | SubmissionStoreError::Revisions(RevisionStoreError::Database(_)) => {
            RecipeStoreError::Unavailable(value.to_string())
        }
        _ => RecipeStoreError::Invalid(value.to_string()),
    }
}
fn storage_io(_: impl std::fmt::Display) -> RecipeStoreError {
    RecipeStoreError::Unavailable(
        "original review storage unavailable; retain original request for readback".into(),
    )
}
pub(crate) async fn submit(
    pool: &Arc<PgPool>,
    actor: &str,
    tenant: &str,
    agent: &str,
    request: SubmitComponentReviewRequest,
) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
    submit_inner(
        pool,
        actor,
        tenant,
        agent,
        request,
        SubmissionIo::compatibility()?,
        None,
        #[cfg(feature = "skills-db")]
        None,
    )
    .await
}
#[cfg(all(feature = "skills-db", test))]
pub(crate) async fn submit_with_catalogue(
    pool: &Arc<PgPool>,
    actor: &str,
    tenant: &str,
    agent: &str,
    request: SubmitComponentReviewRequest,
    catalogue: &Arc<crate::installed_monty_catalogue::InstalledMontyCatalogue>,
) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
    submit_with_catalogue_prepared(pool, actor, tenant, agent, request, catalogue, None).await
}
#[cfg(feature = "skills-db")]
pub(crate) async fn submit_with_catalogue_prepared(
    pool: &Arc<PgPool>,
    actor: &str,
    tenant: &str,
    agent: &str,
    request: SubmitComponentReviewRequest,
    catalogue: &Arc<crate::installed_monty_catalogue::InstalledMontyCatalogue>,
    preparation: Option<brassclaw_host_api::task_preparation::RetainedPreparation>,
) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
    let io = SubmissionIo::capture(
        catalogue
            .resource_store_settings
            .snapshot()
            .map_err(storage_io)?,
    )?;
    submit_inner(
        pool,
        actor,
        tenant,
        agent,
        request,
        io,
        preparation,
        Some(catalogue),
    )
    .await
}
async fn submit_inner(
    pool: &Arc<PgPool>,
    actor: &str,
    tenant: &str,
    agent: &str,
    request: SubmitComponentReviewRequest,
    io: SubmissionIo,
    accepted_preparation: Option<brassclaw_host_api::task_preparation::RetainedPreparation>,
    #[cfg(feature = "skills-db")] catalogue: Option<
        &Arc<crate::installed_monty_catalogue::InstalledMontyCatalogue>,
    >,
) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
    let queue_ids = request
        .queue_ids
        .iter()
        .map(|id| identifier(id))
        .collect::<Result<Vec<_>, _>>()?;
    let id = identifier(&request.submission_id)?;
    // Resolve the original identity BEFORE fresh settings or worker inspection.
    // The connection is released before Source preparation (pool size one).
    let existing = io
        .run("original_identity_read", async {
            let mut client = crate::pg_transaction_client::TransactionClient::from_client(
                pool.get().await.map_err(storage_io)?,
            );
            let tx = client
                .build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(storage_io)?;
            let original = PgReviewSubmissionStore::read_in_transaction(&tx, id)
                .await
                .map_err(storage)?;
            tx.commit().await.map_err(storage_io)?;
            client.recycle_after_commit();
            Ok::<_, RecipeStoreError>(original)
        })
        .await
        .map_err(|_| storage_io("original identity read deadline"))??;
    let preparation = if let Some(original) = &existing {
        original.preparation.clone()
    } else if let Some(accepted) = accepted_preparation {
        Some(accepted)
    } else {
        #[cfg(feature = "skills-db")]
        {
            match catalogue {
                Some(owner) => Some(
                    io.run("capture_preparation", owner.capture_task_preparation())
                        .await
                        .map_err(|_| storage_io("preparation metadata deadline"))?
                        .map_err(storage_io)?
                        .original()
                        .clone(),
                ),
                // Explicit legacy/non-global store profile; no invented ACK.
                None => None,
            }
        }
        #[cfg(not(feature = "skills-db"))]
        {
            None
        }
    };
    let candidate_policy = existing
        .as_ref()
        .and_then(|original| original.snapshot.revisions().get(&original.candidate.uuid))
        .map(|r| r.draft().preparation())
        .unwrap_or(preparation.as_ref());
    let candidate = ComponentRevisionDraft::from_json_with_preparation(
        &request.candidate_bytes,
        candidate_policy,
    )
    .map_err(|error| revision_parse_error(error, "candidate"))?;
    let base = request.base.as_ref().map(reference).transpose()?;
    let dependencies = request
        .dependencies
        .iter()
        .map(reference)
        .collect::<Result<Vec<_>, _>>()?;
    let partner = match request.function_partner {
        Some(partner) => {
            if request.expected_previous_version.is_none() {
                return Err(RecipeStoreError::Invalid(
                    "function review requires each changed allocation head".into(),
                ));
            }
            Some(match partner {
                ComponentFunctionReviewPartner::Changed {
                    candidate_bytes,
                    base,
                    expected_previous_version,
                } => FunctionReviewPartnerDraft::Changed {
                    candidate: Box::new(
                        ComponentRevisionDraft::from_json_with_preparation(
                            &candidate_bytes,
                            existing
                                .as_ref()
                                .and_then(|original| {
                                    original
                                        .function_partner
                                        .filter(|partner| partner.changed)
                                        .and_then(|partner| {
                                            original
                                                .snapshot
                                                .revisions()
                                                .get(&partner.reference.uuid)
                                        })
                                })
                                .map(|r| r.draft().preparation())
                                .unwrap_or(preparation.as_ref()),
                        )
                        .map_err(|error| revision_parse_error(error, "partner"))?,
                    ),
                    base: base.as_ref().map(reference).transpose()?,
                    expected_previous_version,
                },
                ComponentFunctionReviewPartner::Context { reference: value } => {
                    FunctionReviewPartnerDraft::Context {
                        reference: reference(&value)?,
                    }
                }
            })
        }
        None => None,
    };
    let draft = if let Some(preparation) = preparation {
        ReviewSubmissionDraft::new_prepared(
            id,
            actor,
            candidate,
            base,
            dependencies,
            brassclaw_skills::review_submission_store::PreparedReviewContext {
                expected_previous_version: request
                    .expected_previous_version
                    .unwrap_or_else(|| base.map_or(0, |b| b.version)),
                partner: partner,
                preparation: preparation,
            },
        )
    } else {
        match (request.expected_previous_version, partner) {
            (Some(head), Some(partner)) => ReviewSubmissionDraft::new_function(
                id,
                actor,
                candidate,
                base,
                dependencies,
                head,
                partner,
            ),
            (Some(head), None) => ReviewSubmissionDraft::new_after_head(
                id,
                actor,
                candidate,
                base,
                dependencies,
                head,
            ),
            (None, None) => ReviewSubmissionDraft::new(id, actor, candidate, base, dependencies),
            (None, Some(_)) => {
                return Err(RecipeStoreError::Invalid(
                    "function review requires allocation heads".into(),
                ));
            }
        }
    }
    .map_err(storage)?;
    if let Some(original) = &existing {
        draft.require_original(original).map_err(storage)?;
    }
    #[cfg(feature = "skills-db")]
    let source_preparation = match catalogue {
        Some(owner) => owner
            .prepare_source_submission(&draft, &io)
            .await
            .map_err(storage_io)?,
        None => None,
    };
    if let Some(project) = request.create_queue_project {
        if project.is_empty()
            || !queue_ids.is_empty()
            || draft.changed_roots().map_err(storage)?.len() != 1
        {
            return Err(RecipeStoreError::Invalid(
                "one explicit new queue projection required".into(),
            ));
        }
        let retained = crate::queued_review_submission::create(
            pool,
            brassclaw_engine::memory::retrieval_source::ComponentScope {
                tenant_id: tenant.into(),
                user_id: actor.into(),
                agent_id: agent.into(),
                project_id: project,
            },
            &draft,
            &io,
            #[cfg(feature = "skills-db")]
            source_preparation.as_ref(),
        )
        .await?;
        #[cfg(feature = "skills-db")]
        if let Some(owner) = catalogue {
            if source_preparation.is_none() {
                io.run(
                    "queue_source_original_read",
                    owner.read_source_submission(retained.id),
                )
                .await
                .map_err(|_| storage_io("original queue source read deadline"))?
                .map_err(storage_io)?;
            }
            if let Some(wake) = owner.private_work_wake.get() {
                wake.wake();
            }
        }
        return io
            .run("receipt_projection", original_receipt(pool, &retained))
            .await
            .map_err(|_| storage_io("original receipt read deadline"))?;
    }
    let write = io.run("authoring_transaction", async {
        let mut client = crate::pg_transaction_client::TransactionClient::from_client(
            pool.get().await.map_err(storage_io)?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(storage_io)?;
        let mut targets = Vec::new();
        // The explicit selector is retained; no component/latest lookup chooses
        // the subject. Locks and complete payload checks happen in the helper.
        for queue_id in &queue_ids {
            let row=tx.query_opt("SELECT tenant_id,user_id,agent_id,project_id,component_id FROM reborn_validation_queue WHERE id=$1",&[queue_id]).await.map_err(storage_io)?;
            if let Some(row) = row {
                targets.push((
                    brassclaw_engine::memory::retrieval_source::ComponentScope {
                        tenant_id: row.get(0),
                        user_id: row.get(1),
                        agent_id: row.get(2),
                        project_id: row.get(3),
                    },
                    row.get(4),
                    *queue_id,
                ));
            } else {
                // Historical original readback is valid after Q2 queue removal.
                let old = brassclaw_skills::review_submission_store::queue::read_in_transaction(
                    &tx, *queue_id,
                )
                .await
                .map_err(storage)?
                .ok_or_else(|| RecipeStoreError::NotFound("queue capture not found".into()))?;
                if old.submission_id() != id {
                    return Err(RecipeStoreError::Conflict(
                        "queue belongs to another original review".into(),
                    ));
                }
            }
        }
        let retained =
            crate::validation_queue::ValidationQueueStore::submit_original_in_transaction(
                &tx,
                &draft,
                &targets,
                &queue_ids,
                if queue_ids.is_empty() {
                    "standalone"
                } else {
                    "existing_queue"
                },
            )
            .await
            .map_err(crate::validation_queue::original_review_error)?;
        #[cfg(feature = "skills-db")]
        if let Some(prepared) = &source_preparation {
            prepared
                .retain(&tx, retained.id)
                .await
                .map_err(storage_io)?;
        }
        #[cfg(feature = "skills-db")]
        if let Some(owner) = catalogue {
            if source_preparation.is_none() {
                owner
                    .verify_source_submission(&tx, retained.id)
                    .await
                    .map_err(storage_io)?;
            }
        }
        io.note("author_commit", "started");
        if tx.commit().await.is_err() {
            io.note("author_commit", "commit_acknowledgement_lost");
            // Never recycle the unacknowledged original connection.
            drop(client);
            Ok::<_,RecipeStoreError>(None)
        } else {
            client.recycle_after_commit();
            Ok(Some(retained))
        }
    }).await;
    let retained = match write {
        Ok(Ok(Some(retained))) => retained,
        Ok(Err(error)) => return Err(error),
        // Deadline preserves uncertainty; a later same-ID request reads the
        // original. Never replay authoring or claim that the write did not run.
        Err(()) => return Err(storage_io("original authoring deadline; outcome unresolved")),
        Ok(Ok(None)) => io.run("authoring_original_readback", async {
            let mut readback = crate::pg_transaction_client::TransactionClient::from_client(
                pool.get().await.map_err(storage_io)?,
            );
            let original = readback
                .build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(storage_io)?;
            let saved = PgReviewSubmissionStore::read_in_transaction(&original, id)
                .await
                .map_err(storage)?
                .ok_or_else(|| storage_io("original commit unresolved"))?;
            draft.require_original(&saved).map_err(storage)?;
            let request = original.query_opt("SELECT queue_ids,request_kind FROM reborn_component_review_queue_requests WHERE submission_id=$1", &[&id])
                .await.map_err(storage_io)?.ok_or_else(|| storage_io("original queue request missing"))?;
            let mut expected_ids = queue_ids.clone();
            expected_ids.sort();
            if request.get::<_, Vec<Uuid>>(0) != expected_ids
                || request.get::<_, String>(1)
                    != if expected_ids.is_empty() {
                        "standalone"
                    } else {
                        "existing_queue"
                    }
            {
                return Err(RecipeStoreError::Conflict(
                    "original queue request differs".into(),
                ));
            }
            for queue_id in &expected_ids {
                let binding =
                    brassclaw_skills::review_submission_store::queue::read_in_transaction(
                        &original, *queue_id,
                    )
                    .await
                    .map_err(storage)?
                    .ok_or_else(|| storage_io("original queue binding missing"))?;
                if binding.submission_id() != saved.id
                    || binding.subject_checksum() != saved.checksum
                {
                    return Err(RecipeStoreError::Conflict(
                        "original queue binding differs".into(),
                    ));
                }
            }
            #[cfg(feature = "skills-db")]
            if let Some(owner) = catalogue {
                owner
                    .verify_source_submission(&original, id)
                    .await
                    .map_err(storage_io)?;
            }
            original.commit().await.map_err(storage_io)?;
            readback.recycle_after_commit();
            Ok::<_,RecipeStoreError>(saved)
        }).await.map_err(|_|storage_io("original commit readback deadline"))??,
    };
    #[cfg(feature = "skills-db")]
    if let Some(owner) = catalogue {
        if let Some(wake) = owner.private_work_wake.get() {
            wake.wake();
        }
    }
    io.run("receipt_projection", original_receipt(pool, &retained))
        .await
        .map_err(|_| storage_io("original receipt read deadline"))?
}
pub(crate) async fn head(
    pool: &Arc<PgPool>,
    component_id: &str,
) -> Result<brassclaw_product_workflow::ComponentRevisionHeadView, RecipeStoreError> {
    let id = identifier(component_id)?;
    let value = brassclaw_skills::revision_store::PgComponentRevisionStore::new(pool.clone())
        .read_allocation_head(id)
        .await
        .map_err(|error| storage(SubmissionStoreError::Revisions(error)))?;
    Ok(brassclaw_product_workflow::ComponentRevisionHeadView {
        component_id: id.to_string(),
        class_code: value.map(|(class, _)| class),
        last_allocated_version: value.map_or(0, |(_, head)| head),
    })
}

pub(crate) async fn get(
    pool: &Arc<PgPool>,
    submission_id: &str,
) -> Result<ComponentReviewSubmissionView, RecipeStoreError> {
    get_inner(
        pool,
        submission_id,
        #[cfg(feature = "skills-db")]
        None,
    )
    .await
}
#[cfg(feature = "skills-db")]
pub(crate) async fn get_with_catalogue(
    pool: &Arc<PgPool>,
    submission_id: &str,
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
) -> Result<ComponentReviewSubmissionView, RecipeStoreError> {
    #[cfg(test)]
    crate::pg_review_submission_io::projection_start();
    let result = async {
        let io = catalogue
            .resource_store_settings
            .snapshot()
            .map_err(|error| {
                #[cfg(test)]
                crate::pg_review_submission_io::projection_error(
                    "SettingsSource",
                    Some("snapshot_failed"),
                    None,
                );
                storage_io(error)
            })?
            .validate()
            .map_err(|error| {
                #[cfg(test)]
                crate::pg_review_submission_io::projection_error("SettingsValidation", None, None);
                storage_io(error)
            })?;
        #[cfg(test)]
        crate::pg_review_submission_io::projection_policy(io);
        let timed = tokio::time::timeout(
            io.timeout,
            get_inner(pool, submission_id, Some(&catalogue.pure_source_review)),
        )
        .await;
        #[cfg(test)]
        if timed.is_err() {
            crate::pg_review_submission_io::projection_deadline();
        }
        timed.map_err(|_| {
            storage_io("original inspection I/O deadline elapsed; outcome unresolved")
        })?
    }
    .await;
    #[cfg(test)]
    crate::pg_review_submission_io::projection_finish(&result);
    result
}
async fn get_inner(
    pool: &Arc<PgPool>,
    submission_id: &str,
    #[cfg(feature = "skills-db")] source: Option<
        &crate::validator_inventory_review::QualifiedPureSourceReview,
    >,
) -> Result<ComponentReviewSubmissionView, RecipeStoreError> {
    let id = identifier(submission_id)?;
    #[cfg(all(test, feature = "skills-db"))]
    crate::pg_review_submission_io::projection_phase("acquire_started", None);
    let mut client = crate::pg_transaction_client::TransactionClient::from_client(
        pool.get().await.map_err(|error| {
            #[cfg(all(test, feature = "skills-db"))]
            crate::pg_review_submission_io::projection_pool_error(&error);
            let _ = error;
            RecipeStoreError::Unavailable("review connection failed".into())
        })?,
    );
    #[cfg(all(test, feature = "skills-db"))]
    crate::pg_review_submission_io::projection_phase("acquire_acknowledged", None);
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(|error| {
            #[cfg(all(test, feature = "skills-db"))]
            crate::pg_review_submission_io::projection_error(
                "Postgres",
                None,
                error.code().map(|code| code.code()),
            );
            let _ = error;
            RecipeStoreError::Unavailable("review snapshot unavailable".into())
        })?;
    #[cfg(all(test, feature = "skills-db"))]
    crate::pg_review_submission_io::projection_phase("begin_acknowledged", None);
    let retained = PgReviewSubmissionStore::read_in_transaction(&tx, id)
        .await
        .map_err(storage)?
        .ok_or_else(|| RecipeStoreError::NotFound("review submission not found".into()))?;
    let saved =
        brassclaw_skills::review_submission_store::selection::read_preparation_in_transaction(
            &tx,
            id,
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .await
        .map_err(storage)?;
    if let Some(saved) = &saved {
        brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage::restore_preparation(saved)
            .map_err(|reason| RecipeStoreError::Invalid(reason.into()))?;
    }
    let review_selection_bytes = saved.map(|saved| saved.bytes().to_owned());
    let mut retained_receipt = receipt(&retained);
    if let Some(row) = tx
        .query_opt(
            "SELECT queue_ids FROM reborn_component_review_queue_requests WHERE submission_id=$1",
            &[&retained.id],
        )
        .await
        .map_err(storage_io)?
    {
        retained_receipt.queue_ids = row
            .get::<_, Vec<Uuid>>(0)
            .into_iter()
            .map(|id| id.to_string())
            .collect();
    }

    if !retained_receipt.queue_ids.is_empty() {
        retained_receipt.queue_binding_status =
            brassclaw_product_workflow::ReviewQueueBindingStatus::Bound;
    }
    #[cfg(all(test, feature = "skills-db"))]
    crate::pg_review_submission_io::projection_phase("work_inspection", None);
    #[cfg(feature = "skills-db")]
    let work = Some(
        crate::review_work_reconciliation::inspect_with_source(&tx, id, source)
            .await
            .map_err(|error| {
                #[cfg(all(test, feature = "skills-db"))]
                crate::pg_review_submission_io::projection_driver_error(&error);
                let _ = error;
                RecipeStoreError::Unavailable("original review work requires reconciliation".into())
            })?,
    );
    #[cfg(not(feature = "skills-db"))]
    let work = None;
    #[cfg(all(test, feature = "skills-db"))]
    crate::pg_review_submission_io::projection_phase("snapshot_commit", None);
    tx.commit().await.map_err(|error| {
        #[cfg(all(test, feature = "skills-db"))]
        crate::pg_review_submission_io::projection_error(
            "Postgres",
            None,
            error.code().map(|code| code.code()),
        );
        let _ = error;
        RecipeStoreError::Unavailable("review snapshot commit unavailable".into())
    })?;
    client.recycle_after_commit();
    Ok(ComponentReviewSubmissionView {
        receipt: retained_receipt,
        subject_bytes: retained.subject_bytes,
        component_bytes: retained
            .snapshot
            .revisions()
            .values()
            .map(|revision| revision.draft().exact_bytes().to_owned())
            .collect(),
        review_selection_bytes,
        work,
    })
}

#[cfg(test)]
#[path = "pg_review_submission_tests.rs"]
mod tests;

#[cfg(all(test, feature = "skills-db"))]
#[path = "pg_review_model_selection_tests.rs"]
mod model_selection_tests;

#[cfg(test)]
#[path = "pg_function_review_submission_tests.rs"]
pub(crate) mod function_tests;

#[cfg(test)]
#[path = "original_queue_binding_tests.rs"]
mod queue_tests;
