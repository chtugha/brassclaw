//! Related model observations under the existing native invocation owner.
//! This module cannot schedule, retry, approve or interpret a review.
use super::*;
use brassclaw_turns::run_profile::{ModelProfileId, ModelWorkKind};

pub(super) struct RelatedModelWork {
    pub(super) request: ModelWorkRequest,
    work_bytes: String,
    // Complete bytes are immutable in the acknowledged native row. Keeping
    // their checked digest avoids retaining another full prompt in host RAM.
    request_checksum: String,
}

async fn original_review_input(
    selected: Arc<dyn LoopTaskModelSelection>,
) -> Result<brassclaw_turns::run_profile::LoopModelInputReference, AgentLoopHostError> {
    #[cfg(feature = "root-llm-provider")]
    {
        Ok(crate::task_model_provider::prefix_from_policy(
            selected,
            brassclaw_llm::ProviderRole::Sempai,
            brassclaw_skills::recipe_model_input::RecipeModelInput::Default,
        )
        .resolve()
        .await?
        .input_reference())
    }
    #[cfg(not(feature = "root-llm-provider"))]
    {
        let _ = selected;
        Err(error())
    }
}

impl Record {
    pub(super) fn has_pending_observations(&self) -> bool {
        self.wire.pending.load(Ordering::Acquire)
            || self.provider.pending.load(Ordering::Acquire)
            || self.response.pending.load(Ordering::Acquire)
    }

    pub(super) fn check_related_provider_identity(
        &self,
        value: &Value,
    ) -> Result<(), AgentLoopHostError> {
        if let Some(work) = &self.related_work {
            let route = work
                .request
                .resolved_model_route
                .as_ref()
                .ok_or_else(error)?;
            if value["provider_id"].as_str() != Some(route.provider_id.as_str())
                || value["model_id"].as_str() != Some(route.model_id.as_str())
            {
                return Err(error());
            }
        }
        Ok(())
    }
}

impl Handle {
    pub(super) async fn prepare_related_record(
        &self,
        work: &ModelWorkRequest,
        request_bytes: &str,
    ) -> Result<Arc<dyn RecipeModelResponseRecord>, AgentLoopHostError> {
        let parent = &self.0;
        let admission = &parent.invocation.admission;
        let chat = admission.chat_origin().map_err(|_| error())?;
        let id = work.work_id.ok_or_else(error)?;
        if parent.related_work.is_some()
            || id.as_uuid() == parent.invocation.id
            || work.kind != ModelWorkKind::SempaiReview
            || work.model_profile_id != ModelProfileId::new("sempai_model").map_err(|_| error())?
            || parent.submission_reserved.load(Ordering::Acquire)
            || parent.wire.observed.load(Ordering::Acquire)
            || parent.provider.observed.load(Ordering::Acquire)
            || parent.response.observed.load(Ordering::Acquire)
        {
            return Err(error());
        }
        let selected = parent.model_selection.as_ref().ok_or_else(error)?;
        let route = selected.role_model_route("sempai")?;
        if work.resolved_model_route.as_ref() != Some(&route) {
            return Err(error());
        }
        // Resolve the reviewer's own original role input through the existing
        // task-family source. This is not the Kohai prefix or output request.
        let input_reference = original_review_input(selected.clone()).await?;
        let limits = parent.invocation.data_limits;
        let request = strict_json(request_bytes, limits).map_err(|_| error())?;
        if request["run_id"]
            != serde_json::to_value(admission.attempt.run_id).map_err(|_| error())?
            || request["turn_id"] != serde_json::to_value(chat.turn_id).map_err(|_| error())?
            || request["model_profile_id"]
                != serde_json::to_value(&work.model_profile_id).map_err(|_| error())?
            || request["resolved_model_route"]
                != serde_json::to_value(&route).map_err(|_| error())?
            || request["dispatch_policy"]
                != serde_json::to_value(
                    brassclaw_turns::run_profile::LoopModelDispatchPolicy::SingleSubmission,
                )
                .map_err(|_| error())?
            || request["messages"].as_array().is_none_or(Vec::is_empty)
            || !request["surface_version"].is_null()
        {
            return Err(error());
        }
        let work_bytes = encode_json(work, limits.max_bytes).map_err(|_| error())?;
        let metadata = selected.retained_metadata()?;
        let mut client = admission.pool.get().await.map_err(|_| error())?;
        let tx = client.transaction().await.map_err(|_| error())?;
        admission.preflight_claim(&tx).await.map_err(|_| error())?;
        let valid = tx.query_opt(
            "SELECT i.phase,a.task_model_selection FROM brassclaw_monty_tool_invocations i
             JOIN brassclaw_monty_task_admissions a ON a.run_id=i.run_id AND a.admission_key=i.admission_key
             WHERE i.invocation_id=$1 AND i.run_id=$2 AND i.admission_key=$3 AND i.invocation_key=$4
               AND i.selection_bytes::jsonb->>'capability_id'='host.kohai_complete' AND a.phase='started'
             FOR UPDATE OF i",
            &[&parent.invocation.id,&admission.attempt.run_id.as_uuid(),&&admission.key[..],&&parent.invocation.key[..]],
        ).await.map_err(|_| error())?.ok_or_else(error)?;
        if valid.get::<_, &str>(0) != "dispatch_intent"
            || valid.get::<_, Option<Value>>(1) != Some(metadata)
            || parent.submission_reserved.load(Ordering::Acquire)
        {
            return Err(error());
        }
        tx.execute(
            "INSERT INTO brassclaw_monty_related_model_work
             (work_id,invocation_id,work_bytes,work_checksum,request_bytes,request_checksum,prefix_reference)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
            &[&id.as_uuid(),&parent.invocation.id,&work_bytes,&digest(&work_bytes),
              &request_bytes,&digest(request_bytes),&input_reference.reference],
        ).await.map_err(|_| error())?;
        admission.verify_claim(&tx).await.map_err(|_| error())?;
        let record = Arc::new(Record {
            invocation: PgMontyInvocation {
                admission: admission.clone(),
                id: parent.invocation.id,
                key: parent.invocation.key,
                data_limits: limits,
            },
            input_reference,
            // The actual review gateway retains its own output producer. A
            // parent's request or a work estimate cannot become that value.
            requested_output_tokens: None,
            model_selection: Some(selected.clone()),
            related_work: Some(RelatedModelWork {
                request: work.clone(),
                work_bytes,
                request_checksum: digest(request_bytes),
            }),
            related: Mutex::new(HashMap::new()),
            wire: Observation::new(),
            provider: Observation::new(),
            response: Observation::new(),
            submission_reserved: AtomicBool::new(false),
        });
        {
            let mut related = parent.related.lock().map_err(|_| error())?;
            if related.contains_key(&id) {
                return Err(error());
            }
            related.try_reserve(1).map_err(|_| error())?;
            related.insert(id, record.clone());
        }
        // Retain the original record before an acknowledgement can be lost.
        // A failed COMMIT cannot authorize a new identity/submission.
        tx.commit().await.map_err(|_| error())?;
        Ok(Arc::new(Handle(record)))
    }
}

impl PgMontyInvocation {
    pub(super) async fn reserve_related_submission(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        work: &RelatedModelWork,
    ) -> Result<(), AgentLoopHostError> {
        let id = work.request.work_id.ok_or_else(error)?;
        let changed = tx.execute(
            "UPDATE brassclaw_monty_related_model_work SET phase='dispatch_intent',dispatch_count=1,
                dispatched_at=clock_timestamp()
             WHERE work_id=$1 AND invocation_id=$2 AND work_bytes=$3 AND request_checksum=$4
               AND phase='prepared' AND dispatch_count=0",
            &[&id.as_uuid(),&self.id,&work.work_bytes,&work.request_checksum],
        ).await.map_err(|_| error())?;
        if changed != 1 {
            return Err(error());
        }
        Ok(())
    }

    pub(super) async fn record_related_evidence(
        &self,
        work: &RelatedModelWork,
        kind: EvidenceKind,
        prefix: &str,
        bytes: &str,
    ) -> Result<(), AgentLoopHostError> {
        let id = work.request.work_id.ok_or_else(error)?;
        if bytes.len() > self.data_limits.max_bytes {
            return Err(error());
        }
        let mut client = self.admission.pool.get().await.map_err(|_| error())?;
        let tx = client.transaction().await.map_err(|_| error())?;
        // Late originals may be persisted after cancellation/claim loss. This
        // verifies their immutable address, never permission for another effect.
        let valid = tx
            .query_opt(
                "SELECT 1 FROM brassclaw_monty_related_model_work w
             JOIN brassclaw_monty_tool_invocations i ON i.invocation_id=w.invocation_id
             WHERE w.work_id=$1 AND w.invocation_id=$2 AND w.work_bytes=$3 AND w.request_checksum=$4
               AND w.prefix_reference=$5 AND w.phase='dispatch_intent' AND w.dispatch_count=1
               AND i.run_id=$6 AND i.admission_key=$7 AND i.invocation_key=$8",
                &[
                    &id.as_uuid(),
                    &self.id,
                    &work.work_bytes,
                    &work.request_checksum,
                    &prefix,
                    &self.admission.attempt.run_id.as_uuid(),
                    &&self.admission.key[..],
                    &&self.key[..],
                ],
            )
            .await
            .map_err(|_| error())?
            .is_some();
        if !valid {
            return Err(error());
        }
        tx.execute(
            "INSERT INTO brassclaw_monty_related_model_observations
             (work_id,format,response_bytes,response_checksum) VALUES($1,$2,$3,$4)
             ON CONFLICT(work_id,format) DO NOTHING",
            &[&id.as_uuid(), &kind.format(), &bytes, &digest(bytes)],
        )
        .await
        .map_err(|_| error())?;
        let row = tx.query_one(
            "SELECT response_bytes,response_checksum FROM brassclaw_monty_related_model_observations
             WHERE work_id=$1 AND format=$2",
            &[&id.as_uuid(),&kind.format()],
        ).await.map_err(|_| error())?;
        if row.get::<_, &str>(0) != bytes || row.get::<_, String>(1) != digest(bytes) {
            return Err(error());
        }
        tx.commit().await.map_err(|_| error())
    }
}
