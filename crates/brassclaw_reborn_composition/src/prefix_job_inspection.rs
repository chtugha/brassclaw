//! Read-only job projection over existing immutable definitions, turns and
//! admission/effect records. Reading cannot complete, recover or retry a task.
use super::*;
use brassclaw_product_workflow::{InterceptorConfigServiceError as Error, PrefixJobDetail};
use serde_json::Value;

impl RebornRuntime {
    pub async fn inspect_prefix_job(
        &self,
        profile: &str,
        id: Uuid,
    ) -> Result<PrefixJobDetail, Error> {
        let unavailable = || Error::Unavailable;
        let pool = self.installed_catalogue.pool.clone();
        let header = crate::prefix_job_definitions::Store::new(pool.clone())
            .header(id)
            .await
            .map_err(|_| unavailable())?
            .filter(|header| header.profile == profile)
            .ok_or(Error::JobNotFound)?;
        let state = match self
            .turn_coordinator
            .get_run_state(GetRunStateRequest {
                scope: header.scope.clone(),
                run_id: TurnRunId::from_uuid(id),
            })
            .await
        {
            Ok(state) => Some(state),
            Err(brassclaw_turns::TurnError::ScopeNotFound) => None,
            Err(_) => return Err(unavailable()),
        };
        if state.as_ref().is_some_and(|state| {
            state.scope != header.scope
                || state.run_id.as_uuid() != id
                || state.source_binding_ref.as_str()
                    != brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING
                || state.actor.as_ref().map(|actor| actor.user_id.as_str())
                    != Some(header.created_by.as_str())
        }) {
            return Err(unavailable());
        }
        let client = pool.get().await.map_err(|_| unavailable())?;
        let row = client.query_opt("SELECT phase,trusted_internal_turn,scope,turn_id,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1", &[&id]).await.map_err(|_| unavailable())?;
        let mut phase = None;
        let mut receipt = None;
        let mut reference = None;
        if let Some(row) = row {
            let state = state.as_ref().ok_or_else(unavailable)?;
            if !row.get::<_, bool>(1)
                || row.get::<_, Value>(2)
                    != serde_json::to_value(&header.scope).map_err(|_| unavailable())?
                || row.get::<_, Uuid>(3) != state.turn_id.as_uuid()
            {
                return Err(unavailable());
            }
            let actual_phase: String = row.get(0);
            let outcome: Option<Value> = row.get(4);
            if let Some(outcome) = &outcome {
                crate::pg_monty_admission::validate_outcome(outcome, true, state.run_id)
                    .map_err(|_| unavailable())?;
            }
            if state.status == TurnStatus::Completed {
                let outcome = outcome
                    .as_ref()
                    .filter(|outcome| actual_phase == "settled" && outcome["status"] == "completed")
                    .ok_or_else(unavailable)?;
                receipt = Some(
                    outcome["internal_receipt"]
                        .as_str()
                        .ok_or_else(unavailable)?
                        .to_owned(),
                );
                reference = Some(
                    outcome["result_ref"]
                        .as_str()
                        .ok_or_else(unavailable)?
                        .to_owned(),
                );
            }
            phase = Some(actual_phase);
        } else if state
            .as_ref()
            .is_some_and(|state| state.status == TurnStatus::Completed)
        {
            return Err(unavailable());
        }
        let calls = client.query_one("SELECT count(*) FILTER (WHERE phase='answered'),count(*) FILTER (WHERE phase='dispatch_intent') FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&id]).await.map_err(|_| unavailable())?;
        Ok(PrefixJobDetail {
            job_id: header.id.to_string(),
            thread_id: header.scope.thread_id.to_string(),
            profile: header.profile,
            profile_revision: header.revision,
            operation: header.operation,
            mode: header.mode,
            created_at: header.created_at,
            request_sha256: header.request_sha256,
            program_sha256: header.program_sha256,
            input_sha256: header.input_sha256,
            run_status: state.as_ref().map(|state| state.status),
            admission_phase: phase,
            answered_tool_calls: u64::try_from(calls.get::<_, i64>(0))
                .map_err(|_| unavailable())?,
            pending_tool_calls: u64::try_from(calls.get::<_, i64>(1)).map_err(|_| unavailable())?,
            failure_kind: state
                .and_then(|state| state.failure)
                .map(|failure| failure.into_category()),
            internal_receipt: receipt,
            result_ref: reference,
        })
    }
}
