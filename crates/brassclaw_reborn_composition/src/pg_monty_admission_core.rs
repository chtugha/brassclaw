// Durable no-replay admission candidate for the global task driver.
//
// The turn store remains claim authority. A locked real snapshot validates its
// exact attempt before reservation, every dispatched host operation and final
// settlement. This receipt grants no Tool permission and supplies no automatic
// retry. Lost/uncertain admissions require explicit recovery, never deletion.

use brassclaw_pg::PgPool;
use brassclaw_turns::{
    AcceptedMessageRef, SourceBindingRef, TurnId, TurnPersistenceSnapshot, TurnScope, TurnStatus,
    run_profile::{AgentLoopDriverError, LoopRunContext, MontyTaskAttempt},
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};
use tokio_postgres::Transaction;

#[path = "pg_monty_invocation.rs"]
mod invocation;
pub(crate) use invocation::PgMontyInvocation;

/// An exact ordinary catalogue selected by its owner. This is retained data,
/// not approval, child provenance or authority. Internal workflows retain their
/// separate program subjects; historical/draft admissions have no such reference.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CatalogueReference {
    pub(crate) id: uuid::Uuid,
    pub(crate) checksum: [u8; 32],
}

/// Private admission address; neither serializable nor diagnostic-printable.
pub(crate) struct PgMontyAdmission {
    pool: Arc<PgPool>,
    origin: AdmissionOrigin,
    attempt: MontyTaskAttempt,
    key: [u8; 32],
    checksum: [u8; 32],
    catalogue: OnceLock<Option<CatalogueReference>>,
    task_models: OnceLock<Option<Value>>,
}
struct ChatAdmissionOrigin {
    scope: TurnScope,
    turn_id: TurnId,
    accepted: AcceptedMessageRef,
    trusted_internal_turn: bool,
    source_binding_ref: Option<SourceBindingRef>,
    actor: Option<brassclaw_turns::TurnActor>,
}
enum AdmissionOrigin {
    Chat(Box<ChatAdmissionOrigin>),
    Private(Arc<brassclaw_turns::private_work::ClaimedPrivateWork>),
}

impl PgMontyAdmission {
    // Native fixture convenience only. Application owners must retain a
    // prepared address before I/O, including an ambiguous COMMIT result.
    #[cfg(test)]
    pub(crate) async fn reserve(
        pool: Arc<PgPool>,
        context: &LoopRunContext,
        attempt: MontyTaskAttempt,
    ) -> Result<Self, AgentLoopDriverError> {
        let reservation = Self::prepare(pool, context, attempt, None)?;
        reservation.persist_reservation().await?;
        Ok(reservation)
    }

    /// Construct the private reservation before database I/O. The production
    /// owner retains this object before calling `persist_reservation`: a lost
    /// commit acknowledgement must not discard the only admission key.
    pub(crate) fn prepare(
        pool: Arc<PgPool>,
        context: &LoopRunContext,
        attempt: MontyTaskAttempt,
        source_binding_ref: Option<&SourceBindingRef>,
    ) -> Result<Self, AgentLoopDriverError> {
        if context.run_id != attempt.run_id
            || source_binding_ref.is_some_and(|source| {
                context.trusted_internal_turn
                    != (source.as_str()
                        == brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING)
            })
        {
            return Err(failed("monty_admission_identity_invalid"));
        }
        let accepted = context
            .accepted_message_ref
            .clone()
            .ok_or_else(|| failed("monty_admission_identity_invalid"))?;
        let claim = serde_json::to_vec(&attempt.lease_token)
            .map_err(|_| failed("monty_admission_identity_invalid"))?;
        let nonce = serde_json::to_vec(&brassclaw_turns::TurnLeaseToken::new())
            .map_err(|_| failed("monty_admission_identity_invalid"))?;
        Ok(Self {
            pool,
            origin: AdmissionOrigin::Chat(Box::new(ChatAdmissionOrigin {
                scope: context.scope.clone(),
                turn_id: context.turn_id,
                accepted,
                trusted_internal_turn: context.trusted_internal_turn,
                source_binding_ref: source_binding_ref.cloned(),
                actor: context.actor.clone(),
            })),
            attempt,
            key: Sha256::digest(nonce).into(),
            checksum: Sha256::digest(claim).into(),
            catalogue: OnceLock::new(),
            task_models: OnceLock::new(),
        })
    }

    pub(crate) fn prepare_private(
        pool: Arc<PgPool>,
        claimed: Arc<brassclaw_turns::private_work::ClaimedPrivateWork>,
    ) -> Result<Self, AgentLoopDriverError> {
        let attempt = claimed.attempt();
        let claim = serde_json::to_vec(&attempt.lease_token)
            .map_err(|_| failed("monty_admission_identity_invalid"))?;
        let nonce = serde_json::to_vec(&brassclaw_turns::TurnLeaseToken::new())
            .map_err(|_| failed("monty_admission_identity_invalid"))?;
        Ok(Self {
            pool,
            origin: AdmissionOrigin::Private(claimed),
            attempt,
            key: Sha256::digest(nonce).into(),
            checksum: Sha256::digest(claim).into(),
            catalogue: OnceLock::new(),
            task_models: OnceLock::new(),
        })
    }
    fn chat_origin(&self) -> Result<&ChatAdmissionOrigin, AgentLoopDriverError> {
        match &self.origin {
            AdmissionOrigin::Chat(origin) => Ok(origin),
            AdmissionOrigin::Private(_) => Err(failed("monty_chat_origin_required")),
        }
    }
    fn original_source_binding(&self) -> Option<&SourceBindingRef> {
        match &self.origin {
            AdmissionOrigin::Chat(origin) => origin.source_binding_ref.as_ref(),
            AdmissionOrigin::Private(_) => None,
        }
    }
    fn is_result_only(&self) -> bool {
        match &self.origin {
            AdmissionOrigin::Private(_) => true,
            AdmissionOrigin::Chat(origin) => {
                origin.trusted_internal_turn
                    || origin.source_binding_ref.as_ref().is_some_and(|source| {
                        source.as_str()
                            == brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING
                    })
            }
        }
    }

    /// Freeze the actual captured ordinary source before the first host call.
    /// Concurrent identical calls coalesce; no later source may replace it,
    /// including replacing an explicitly absent source with the current one.
    pub(crate) fn bind_catalogue(
        &self,
        reference: Option<CatalogueReference>,
    ) -> Result<(), AgentLoopDriverError> {
        if reference.is_some_and(|reference| reference.id.is_nil()) {
            return Err(failed("monty_catalogue_reference_invalid"));
        }
        match self.catalogue.set(reference) {
            Ok(()) => Ok(()),
            Err(reference) if self.catalogue.get() == Some(&reference) => Ok(()),
            Err(_) => Err(failed("monty_catalogue_reference_conflict")),
        }
    }

    /// Pin metadata from the real task model port before effects. It carries
    /// references, not authority; never backfill an old admission from latest.
    pub(crate) fn bind_task_models(
        &self,
        selection: Option<Value>,
    ) -> Result<(), AgentLoopDriverError> {
        if selection.as_ref().is_some_and(|selection| {
            let format = selection["format"].as_str();
            let fields = match format {
                Some("brassclaw-task-provider-binding/1") => 3,
                Some("brassclaw-task-provider-binding/2") => 4,
                Some("brassclaw-task-provider-binding/3") => 5,
                _ => return true,
            };
            !selection["selection"].is_object()
                || selection.as_object().is_none_or(|s| s.len() != fields)
                || (fields >= 4 && !selection["prefix_bindings"].is_array())
                || (fields == 5
                    && (selection["prefix_bindings"]
                        .as_array()
                        .is_none_or(|p| !p.is_empty())
                        || selection["selection"]["namespace"]
                            .as_str()
                            .is_none_or(|namespace| {
                                crate::task_model_capture_record::PrefixFreeCaptureRecord::parse(
                                    &selection["prefix_capture"],
                                    namespace,
                                )
                                .is_none()
                            })))
                || selection["checksum"].as_str().is_none_or(|checksum| {
                    checksum.len() != 64
                        || !checksum
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
        }) {
            return Err(failed("monty_task_model_reference_invalid"));
        }
        if let Some(selection) = &selection {
            let mut document = selection["selection"].clone();
            document.sort_all_objects();
            let mut checksum = hex::encode(Sha256::digest(
                serde_json::to_vec(&document)
                    .map_err(|_| failed("monty_task_model_reference_invalid"))?,
            ));
            if matches!(
                selection["format"].as_str(),
                Some("brassclaw-task-provider-binding/2" | "brassclaw-task-provider-binding/3")
            ) {
                let mut prefixes = selection["prefix_bindings"].clone();
                prefixes.sort_all_objects();
                let mut subject = serde_json::json!([checksum, prefixes]);
                if selection["format"] == "brassclaw-task-provider-binding/3" {
                    let mut capture = selection["prefix_capture"].clone();
                    capture.sort_all_objects();
                    subject
                        .as_array_mut()
                        .ok_or_else(|| failed("monty_task_model_reference_invalid"))?
                        .push(capture);
                }
                checksum = hex::encode(Sha256::digest(
                    serde_json::to_vec(&subject)
                        .map_err(|_| failed("monty_task_model_reference_invalid"))?,
                ));
            }
            if selection["checksum"].as_str() != Some(checksum.as_str()) {
                return Err(failed("monty_task_model_reference_invalid"));
            }
        }
        match self.task_models.set(selection) {
            Ok(()) => Ok(()),
            Err(selection) if self.task_models.get() == Some(&selection) => Ok(()),
            Err(_) => Err(failed("monty_task_model_reference_conflict")),
        }
    }

    /// Persist this exact prepared address once. A conflict is not permission
    /// to replace another admission; an uncertain result requires recovery with
    /// this retained address, never another reservation or effect replay.
    pub(crate) async fn persist_reservation(&self) -> Result<(), AgentLoopDriverError> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        if let AdmissionOrigin::Private(claimed) = &self.origin {
            self.verify_claim(&transaction).await?;
            let retained: bool = transaction
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM brassclaw_monty_task_admissions
                WHERE run_id=$1 AND private_work_id=$2 AND admission_key=$3 AND claim_checksum=$4
                AND origin_kind='private_work' AND phase='reserved')",
                    &[
                        &self.attempt.run_id.as_uuid(),
                        &claimed.request().work_id(),
                        &&self.key[..],
                        &&self.checksum[..],
                    ],
                )
                .await
                .map_err(|_| failed("monty_admission_database_failed"))?
                .get(0);
            if !retained {
                return Err(failed("monty_admission_replay_requires_recovery"));
            }
            transaction
                .commit()
                .await
                .map_err(|_| failed("monty_admission_database_failed"))?;
            return Ok(());
        }
        let chat = self.chat_origin()?;
        self.preflight_claim(&transaction).await?;
        let scope = serde_json::to_value(&chat.scope)
            .map_err(|_| failed("monty_admission_identity_invalid"))?;
        let actor = serde_json::to_value(&chat.actor)
            .map_err(|_| failed("monty_admission_identity_invalid"))?;
        let runner = serde_json::to_value(self.attempt.runner_id)
            .map_err(|_| failed("monty_admission_identity_invalid"))?
            .as_str()
            .ok_or_else(|| failed("monty_admission_identity_invalid"))?
            .to_owned();
        let source_binding = chat
            .source_binding_ref
            .as_ref()
            .map(SourceBindingRef::as_str);
        let inserted = transaction.execute("INSERT INTO brassclaw_monty_task_admissions
            (run_id, turn_id, scope, accepted_message_ref, runner_id, claim_checksum, admission_key, phase, trusted_internal_turn,
             catalogue_reference_version, task_model_selection_version,native_actor,source_binding_ref)
            VALUES ($1,$2,$3,$4,$5::text::uuid,$6,$7,'reserved',$8,1,1,$9,$10) ON CONFLICT (run_id) DO NOTHING",
            &[&self.attempt.run_id.as_uuid(), &chat.turn_id.as_uuid(), &scope, &chat.accepted.as_str(),
                &runner, &&self.checksum[..], &&self.key[..], &chat.trusted_internal_turn,&actor,&source_binding])
            .await.map_err(|_| failed("monty_admission_database_failed"))?;
        if inserted != 1 {
            return Err(failed("monty_admission_replay_requires_recovery"));
        }
        self.verify_claim(&transaction).await?;
        transaction
            .commit()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        drop(client);
        Ok(())
    }

    /// Call before one requested operation. Started/settled admission is not an
    /// operation approval; current kernel policy is independently checked later.
    pub(crate) async fn check_and_start(&self) -> Result<(), AgentLoopDriverError> {
        // Direct internal/qualification callers explicitly freeze absence too.
        // An unconfigured old admission cannot later acquire a fresh selection.
        self.bind_catalogue(self.catalogue.get().copied().flatten())?;
        self.bind_task_models(self.task_models.get().cloned().flatten())?;
        let task_models = self.task_models.get().cloned().flatten();
        let reference = self.catalogue.get().copied().flatten();
        let catalogue_id = reference.map(|reference| reference.id);
        let catalogue_checksum = reference.map(|reference| hex::encode(reference.checksum));
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        self.preflight_claim(&transaction).await?;
        let changed = transaction.execute("UPDATE brassclaw_monty_task_admissions
            SET phase='started', started_at=COALESCE(started_at,clock_timestamp()),
                catalogue_id=COALESCE(catalogue_id,$4),
                catalogue_checksum=COALESCE(catalogue_checksum,$5),
                task_model_selection=COALESCE(task_model_selection,$6)
            WHERE run_id=$1 AND admission_key=$2 AND claim_checksum=$3 AND phase IN ('reserved','started')
                AND ((catalogue_id IS NULL AND catalogue_checksum IS NULL
                      AND ($4::uuid IS NULL OR (phase='reserved' AND catalogue_reference_version=1)))
                     OR (catalogue_id=$4 AND catalogue_checksum=$5))
                AND (task_model_selection IS NOT DISTINCT FROM $6::jsonb
                     OR (task_model_selection IS NULL AND phase='reserved' AND task_model_selection_version=1))",
            &[&self.attempt.run_id.as_uuid(), &&self.key[..], &&self.checksum[..],
                &catalogue_id, &catalogue_checksum, &task_models])
            .await.map_err(|error| {
                tracing::warn!(sqlstate = error.code().map(|code| code.code()),
                    constraint = error.as_db_error().and_then(|error| error.constraint()),
                    "original Monty admission start failed");
                failed("monty_admission_database_failed")
            })?;
        if changed != 1 {
            return Err(failed("monty_admission_fenced"));
        }
        self.verify_claim(&transaction).await?;
        transaction
            .commit()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))
    }

    /// Only the verified actual root outcome belongs here. This method records
    /// local settlement and does not declare external effects reconciled.
    pub(crate) async fn settle(&self, outcome: Value) -> Result<(), AgentLoopDriverError> {
        if matches!(&self.origin, AdmissionOrigin::Private(_)) {
            return Err(failed("private_settlement_requires_native_receipt"));
        }
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        self.settle_in_transaction(&transaction, &outcome).await?;
        transaction
            .commit()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))
    }

    async fn settle_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        outcome: &Value,
    ) -> Result<(), AgentLoopDriverError> {
        let result_only = self.is_result_only();
        validate_outcome(outcome, result_only, self.attempt.run_id)?;
        // Settlement records the original worker's acknowledged outcome; it
        // grants no dispatch or reply authority. Cancellation, lease expiry or
        // reclaim must not prevent preserving this original admission's audit
        // evidence. Its private key/checksum and outcome CAS still fence it.
        let row = transaction
            .query_opt(
                "SELECT phase, outcome, source_binding_ref, result_only_completion
            FROM brassclaw_monty_task_admissions
            WHERE run_id=$1 AND admission_key=$2 AND claim_checksum=$3 FOR UPDATE",
                &[
                    &self.attempt.run_id.as_uuid(),
                    &&self.key[..],
                    &&self.checksum[..],
                ],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?
            .ok_or_else(|| failed("monty_admission_fenced"))?;
        if row.get::<_, Option<String>>(2).as_deref()
            != self.original_source_binding().map(SourceBindingRef::as_str)
            || row.get::<_, bool>(3) != result_only
        {
            return Err(failed("monty_admission_fenced"));
        }
        let phase: &str = row.get(0);
        if phase == "settled" {
            if row.get::<_, Option<Value>>(1).as_ref() != Some(outcome) {
                return Err(failed("monty_admission_settlement_conflict"));
            }
        } else if phase == "started"
            || (phase == "reserved"
                && outcome.get("reason_kind").and_then(Value::as_str) == Some("task_cancelled"))
        {
            let changed = transaction
                .execute(
                    "UPDATE brassclaw_monty_task_admissions
                SET phase='settled', outcome=$3, settled_at=clock_timestamp()
                WHERE run_id=$1 AND admission_key=$2 AND phase IN ('reserved','started')",
                    &[&self.attempt.run_id.as_uuid(), &&self.key[..], outcome],
                )
                .await
                .map_err(|_| failed("monty_admission_database_failed"))?;
            if changed != 1 {
                return Err(failed("monty_admission_fenced"));
            }
        } else {
            return Err(failed("monty_admission_not_started"));
        }
        Ok(())
    }

    async fn verify_claim(
        &self,
        transaction: &Transaction<'_>,
    ) -> Result<(), AgentLoopDriverError> {
        self.inspect_claim(transaction, true).await.map(|_| ())
    }

    /// Reject stale preparation without holding the cancellation/heartbeat
    /// snapshot lock through effect-free journal writes. This is not the final
    /// dispatch fence: every caller must verify again under lock before COMMIT.
    async fn preflight_claim(
        &self,
        transaction: &Transaction<'_>,
    ) -> Result<(), AgentLoopDriverError> {
        self.inspect_claim(transaction, false).await.map(|_| ())
    }

    async fn inspect_claim(
        &self,
        transaction: &Transaction<'_>,
        retain_lock: bool,
    ) -> Result<Option<brassclaw_turns::TurnRunId>, AgentLoopDriverError> {
        if let AdmissionOrigin::Private(claimed) = &self.origin {
            self.inspect_private_claim(transaction, claimed, retain_lock)
                .await?;
            return Ok(None);
        }
        let chat = self.chat_origin()?;
        // Same snapshot key/lock as PgTurnStateStore. The snapshot row lock
        // fences the final claim check and COMMIT, after all effect-free journal
        // preparation. A blocked journal must not hold up cancellation or lease
        // renewal. Cancellation that wins during preparation rolls back every
        // provisional write; no Tool can run without the verified commit.
        let query = "SELECT payload FROM brassclaw_turns
            WHERE tenant_id=$1 AND status='snapshot' AND turn_id=
            COALESCE((SELECT snapshot_thread_id FROM brassclaw_turn_snapshot_threads
                WHERE tenant_id=$1 AND thread_id=$2),$2)";
        let query = if retain_lock {
            format!("{query} FOR UPDATE")
        } else {
            query.to_owned()
        };
        let row = transaction
            .query_opt(
                &query,
                &[
                    &chat.scope.tenant_id.as_str(),
                    &chat.scope.thread_id.as_str(),
                ],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?
            .ok_or_else(|| failed("monty_admission_fenced"))?;
        let snapshot: TurnPersistenceSnapshot = serde_json::from_value(row.get::<_, Value>(0))
            .map_err(|_| failed("monty_admission_snapshot_invalid"))?;
        let now: DateTime<Utc> = transaction
            .query_one("SELECT clock_timestamp()", &[])
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?
            .get(0);
        let run = snapshot
            .runs
            .iter()
            .find(|run| run.run_id == self.attempt.run_id)
            .ok_or_else(|| failed("monty_admission_fenced"))?;
        if run.scope != chat.scope
            || chat.actor.as_ref().is_some_and(|actor| {
                !snapshot.turns.iter().any(|turn| {
                    turn.turn_id == chat.turn_id
                        && turn.scope == chat.scope
                        && &turn.actor == actor
                        && turn.accepted_message_ref == chat.accepted
                })
            })
            || run.turn_id != chat.turn_id
            || run.accepted_message_ref != chat.accepted
            || run.runner_id != Some(self.attempt.runner_id)
            || run.lease_token != Some(self.attempt.lease_token)
            || run
                .lease_expires_at
                .as_ref()
                .is_none_or(|expires| *expires <= now)
            || chat.trusted_internal_turn
                != (run.source_binding_ref.as_str()
                    == brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING)
            || chat
                .source_binding_ref
                .as_ref()
                .is_some_and(|source| source != &run.source_binding_ref)
            || (chat.source_binding_ref.is_none()
                && run.source_binding_ref.as_str()
                    == brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING)
            || run.status != TurnStatus::Running
        {
            return Err(failed("monty_admission_fenced"));
        }
        Ok(run.parent_run_id)
    }
}

#[path = "pg_monty_private_admission.rs"]
mod private_origin;
pub(crate) use private_origin::DeliveredPrivateCompletion;

/// Shape validation is not completion proof. The private task owner verifies
/// native receipts before settlement; the evidence reader then checks that exact
/// durable settlement and its scope. A caller-provided marker grants nothing.
pub(super) fn validate_outcome(
    value: &Value,
    result_only: bool,
    run: brassclaw_turns::TurnRunId,
) -> Result<(), AgentLoopDriverError> {
    let invalid = || failed("monty_admission_outcome_invalid");
    let object = value.as_object().ok_or_else(invalid)?;
    let execution = object.get("execution");
    let internal =
        execution.is_some_and(|report| report["format"] == "monty-internal-task-execution/1");
    if (internal && !result_only) || (result_only && value["status"] == "completed" && !internal) {
        return Err(invalid());
    }
    if let Some(report) = execution {
        brassclaw_skills::value_contract::validate_data_bounds(
            report,
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .map_err(|_| invalid())?;
        if (!internal && report["format"] != "monty-task-execution/1")
            || report["semantic_approval"] != false
            || report["catalogue_activation"] != false
            || report["root_completed"] != (value["status"] == "completed")
            || !report["recipes"].is_array()
            || (value["status"] == "completed" && report["all_selected_recipes_complete"] != true)
        {
            return Err(invalid());
        }
    }
    let fields = if execution.is_some() { 3 } else { 2 };
    match object.get("status").and_then(Value::as_str) {
        Some("completed") if internal && object.len() == 4 => {
            let receipt = value["internal_receipt"]
                .as_str()
                .filter(|receipt| !receipt.is_empty())
                .ok_or_else(invalid)?;
            let reference = crate::monty_result_reference::result_reference(run, receipt)
                .ok_or_else(invalid)?;
            let report = execution.ok_or_else(invalid)?;
            if value["result_ref"] != reference.as_str()
                || report["internal_receipt"] != receipt
                || !report["reply"].is_null()
                || report["withheld_root_answers"] != 0
                || report["withheld_host_answers"] != false
                || !report["accounting"].is_object()
                || report["accounting"]["compute_time"].is_null()
                || !report["accounting"]["failure"].is_null()
            {
                return Err(invalid());
            }
        }
        Some("completed") if !internal && object.len() == fields => {
            let reference = value["reply_ref"].as_str().ok_or_else(invalid)?;
            brassclaw_turns::LoopMessageRef::new(reference).map_err(|_| invalid())?;
        }
        Some("failed") if object.len() == fields => {
            let reason = value["reason_kind"].as_str().ok_or_else(invalid)?;
            if reason.is_empty()
                || reason.len() > 64
                || !reason
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

fn failed(reason: &str) -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: reason.into(),
    }
}
