//! Read-only job projection over existing immutable definitions, turns and
//! admission/effect records. Reading cannot complete, recover or retry a task.
use super::*;
use brassclaw_product_workflow::{
    InterceptorConfigServiceError as Error, PrefixJobDetail, PrefixJobTarget,
};
use serde_json::Value;

enum Lookup<'a> {
    ModelSetup {
        specification: &'a str,
        setup_revision: u64,
    },
    Profile(&'a str),
    Pair {
        specification: &'a str,
        proposal: &'a str,
    },
}

impl RebornRuntime {
    pub async fn inspect_prefix_job(
        &self,
        profile: &str,
        id: Uuid,
    ) -> Result<PrefixJobDetail, Error> {
        self.inspect_prefix_job_target(Lookup::Profile(profile), id)
            .await
    }

    pub async fn inspect_prefix_pair_job(
        &self,
        specification: &str,
        proposal: &str,
        id: Uuid,
    ) -> Result<PrefixJobDetail, Error> {
        crate::prefix_model_designs::hash(specification)?;
        crate::prefix_model_designs::hash(proposal)?;
        self.inspect_prefix_job_target(
            Lookup::Pair {
                specification,
                proposal,
            },
            id,
        )
        .await
    }

    pub async fn inspect_prefix_model_setup_job(
        &self,
        specification: &str,
        setup_revision: u64,
        id: Uuid,
    ) -> Result<PrefixJobDetail, Error> {
        crate::prefix_model_designs::hash(specification)?;
        if setup_revision > brassclaw_product_workflow::PREFIX_SETUP_MAX_EXACT_INTEGER {
            return Err(crate::prefix_model_designs::bad());
        }
        self.inspect_prefix_job_target(
            Lookup::ModelSetup {
                specification,
                setup_revision,
            },
            id,
        )
        .await
    }

    async fn inspect_prefix_job_target(
        &self,
        lookup: Lookup<'_>,
        id: Uuid,
    ) -> Result<PrefixJobDetail, Error> {
        let unavailable = || Error::Unavailable;
        let pool = self.installed_catalogue.pool.clone();
        let header = crate::prefix_job_definitions::Store::new(pool.clone())
            .header(id)
            .await
            .map_err(|_| unavailable())?
            .filter(|header| {
                header.scope.tenant_id.as_str() == self.webui_tenant_id()
                    && match (&header.target, &lookup) {
                        (
                            crate::prefix_job_definitions::Target::Profile { name, .. },
                            Lookup::Profile(profile),
                        ) => name.as_str() == *profile,
                        (
                            crate::prefix_job_definitions::Target::Pair {
                                specification,
                                proposal,
                            },
                            Lookup::Pair {
                                specification: expected_specification,
                                proposal: expected_proposal,
                            },
                        ) => {
                            specification.as_str() == *expected_specification
                                && proposal.as_str() == *expected_proposal
                        }
                        (
                            crate::prefix_job_definitions::Target::ModelSetup {
                                specification,
                                setup_revision,
                            },
                            Lookup::ModelSetup {
                                specification: expected,
                                setup_revision: revision,
                            },
                        ) => specification.as_str() == *expected && setup_revision == revision,
                        _ => false,
                    }
            })
            .ok_or(Error::JobNotFound)?;
        let target = match &header.target {
            crate::prefix_job_definitions::Target::Profile { name, revision } => {
                PrefixJobTarget::Profile {
                    profile: name.clone(),
                    profile_revision: *revision,
                }
            }
            crate::prefix_job_definitions::Target::Pair {
                specification,
                proposal,
            } => PrefixJobTarget::Pair {
                specification: specification.clone(),
                proposal: proposal.clone(),
            },
            crate::prefix_job_definitions::Target::ModelSetup {
                specification,
                setup_revision,
            } => PrefixJobTarget::ModelSetup {
                specification: specification.clone(),
                setup_revision: *setup_revision,
            },
        };
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
            target,
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
