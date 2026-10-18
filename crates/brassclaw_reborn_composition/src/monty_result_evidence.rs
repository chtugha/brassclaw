//! Durable internal-result verification over the existing admission owner.
//! No new result registry, task queue, Tool grant or workflow decision lives here.
use crate::monty_result_reference::{PREFIX, result_reference};
use async_trait::async_trait;
use brassclaw_pg::PgPool;
use brassclaw_turns::{
    LoopCheckpointKind, TurnError, TurnId, TurnRunId, TurnScope,
    loop_exit::{
        BlockedEvidenceRequest, CompletionEvidenceRequest, FailureEvidenceRequest,
        FinalCheckpointEvidenceRequest, LoopExitEvidencePort,
    },
};
use serde_json::Value;
use std::sync::Arc;

pub(crate) struct MontyResultEvidence {
    pool: Arc<PgPool>,
    fallback: Arc<dyn LoopExitEvidencePort>,
}
impl MontyResultEvidence {
    pub(crate) fn new(pool: Arc<PgPool>, fallback: Arc<dyn LoopExitEvidencePort>) -> Self {
        Self { pool, fallback }
    }
    async fn verify_internal(
        &self,
        request: CompletionEvidenceRequest<'_>,
    ) -> Result<bool, TurnError> {
        if !request.reply_message_refs.is_empty() || request.result_refs.len() != 1 {
            return Ok(false);
        }
        let unavailable = || TurnError::Unavailable {
            reason: "internal completion evidence unavailable".into(),
        };
        let client = self.pool.get().await.map_err(|_| unavailable())?;
        let row=client.query_opt("SELECT turn_id,scope,phase,trusted_internal_turn,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",&[&request.run_id.as_uuid()]).await.map_err(|_| unavailable())?;
        let Some(row) = row else {
            return Ok(false);
        };
        let scope: Value = row.get(1);
        let outcome: Option<Value> = row.get(4);
        let Some(outcome) = outcome else {
            return Ok(false);
        };
        if crate::pg_monty_admission::validate_outcome(&outcome, row.get(3), request.run_id)
            .is_err()
        {
            return Ok(false);
        }

        let Some(receipt) = outcome["internal_receipt"]
            .as_str()
            .filter(|receipt| !receipt.is_empty())
        else {
            return Ok(false);
        };
        let Some(reference) = result_reference(request.run_id, receipt) else {
            return Ok(false);
        };
        let report = &outcome["execution"];
        Ok(row.get::<_, uuid::Uuid>(0) == request.turn_id.as_uuid()
            && scope == serde_json::to_value(request.scope).map_err(|_| unavailable())?
            && row.get::<_, String>(2) == "settled"
            && row.get::<_, bool>(3)
            && outcome["status"] == "completed"
            && outcome["result_ref"] == reference.as_str()
            && request.result_refs[0] == reference
            && outcome["reply_ref"].is_null()
            && report["format"] == "monty-internal-task-execution/1"
            && report["root_completed"] == true
            && report["all_selected_recipes_complete"] == true
            && report["internal_receipt"] == receipt
            && report["reply"].is_null()
            && report["withheld_root_answers"] == 0
            && report["withheld_host_answers"] == false
            && report["accounting"].is_object()
            && !report["accounting"]["compute_time"].is_null()
            && report["accounting"]["failure"].is_null())
    }
}
#[async_trait]
impl LoopExitEvidencePort for MontyResultEvidence {
    async fn verify_completion_refs(
        &self,
        request: CompletionEvidenceRequest<'_>,
    ) -> Result<bool, TurnError> {
        if request
            .result_refs
            .iter()
            .any(|reference| reference.as_str().starts_with(PREFIX))
        {
            return self.verify_internal(request).await;
        }
        self.fallback.verify_completion_refs(request).await
    }
    async fn verify_final_checkpoint(
        &self,
        request: FinalCheckpointEvidenceRequest<'_>,
    ) -> Result<bool, TurnError> {
        self.fallback.verify_final_checkpoint(request).await
    }
    async fn verify_blocked_evidence(
        &self,
        request: BlockedEvidenceRequest<'_>,
    ) -> Result<bool, TurnError> {
        self.fallback.verify_blocked_evidence(request).await
    }
    async fn verify_failure_evidence(
        &self,
        request: FailureEvidenceRequest<'_>,
    ) -> Result<bool, TurnError> {
        self.fallback.verify_failure_evidence(request).await
    }
    async fn is_cancellation_observed(
        &self,
        scope: &TurnScope,
        turn_id: TurnId,
        run_id: TurnRunId,
    ) -> Result<bool, TurnError> {
        self.fallback
            .is_cancellation_observed(scope, turn_id, run_id)
            .await
    }
    async fn latest_checkpoint_kind(
        &self,
        scope: &TurnScope,
        turn_id: TurnId,
        run_id: TurnRunId,
    ) -> Result<Option<LoopCheckpointKind>, TurnError> {
        self.fallback
            .latest_checkpoint_kind(scope, turn_id, run_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_turns::LoopResultRef;
    use serde_json::json;
    #[test]
    fn internal_receipt_references_are_typed_stable_and_run_bound() {
        let run = TurnRunId::new();
        let receipt = "prefix-generation:base-prompt:é\r\n";
        let reference = result_reference(run, receipt).unwrap();
        assert_eq!(LoopResultRef::new(reference.as_str()).unwrap(), reference);
        assert_eq!(Some(reference.clone()), result_reference(run, receipt));
        assert_ne!(
            Some(reference.clone()),
            result_reference(TurnRunId::new(), receipt)
        );
        assert_ne!(
            Some(reference),
            result_reference(run, "prefix-generation:base-prompt:é\n")
        );
        assert!(result_reference(run, "").is_none());
    }
    #[test]
    fn admission_shapes_distinguish_internal_results_from_chat_publication() {
        let run = TurnRunId::new();
        let receipt = "fixture-result/1";
        let reference = result_reference(run, receipt).unwrap();
        let value = json!({"status":"completed","internal_receipt":receipt,"result_ref":reference.as_str(),
            "execution":{"format":"monty-internal-task-execution/1","semantic_approval":false,"catalogue_activation":false,"root_completed":true,
                "all_selected_recipes_complete":true,"recipes":[{"composed":true,"complete":true}],
                "internal_receipt":receipt,"reply":null,"withheld_root_answers":0,"withheld_host_answers":false,
                "accounting":{"compute_time":{"secs":0,"nanos":1},"failure":null}}});
        // These are shape checks only. Evidence still requires an actual
        // privately settled admission and its exact scope in PostgreSQL.
        assert!(crate::pg_monty_admission::validate_outcome(&value, true, run).is_ok());
        assert!(crate::pg_monty_admission::validate_outcome(&value, false, run).is_err());
        assert!(
            crate::pg_monty_admission::validate_outcome(&value, true, TurnRunId::new()).is_err()
        );
        for (field, bad) in [
            ("root_completed", json!(false)),
            ("semantic_approval", json!(true)),
            ("catalogue_activation", json!(true)),
            ("withheld_root_answers", json!(1)),
            ("withheld_host_answers", json!(true)),
            ("internal_receipt", json!("different")),
            ("reply", json!({"reference":"msg:forged"})),
        ] {
            let mut altered = value.clone();
            altered["execution"][field] = bad;
            assert!(
                crate::pg_monty_admission::validate_outcome(&altered, true, run).is_err(),
                "{field}"
            );
        }
        let mut mixed = value.clone();
        mixed["reply_ref"] = json!("msg:forged");
        assert!(crate::pg_monty_admission::validate_outcome(&mixed, true, run).is_err());
        assert!(
            crate::pg_monty_admission::validate_outcome(
                &json!({"status":"completed","reply_ref":"msg:actual"}),
                false,
                run
            )
            .is_ok()
        );
        assert!(
            crate::pg_monty_admission::validate_outcome(
                &json!({"status":"failed","reason_kind":"task_cancelled"}),
                false,
                run
            )
            .is_ok()
        );
    }
}
