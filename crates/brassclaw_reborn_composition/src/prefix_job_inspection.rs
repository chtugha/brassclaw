//! Read-only job projection over existing immutable definitions, turns and
//! admission/effect records. Reading cannot complete, recover or retry a task.
use super::*;
use brassclaw_product_workflow::{
    InterceptorConfigServiceError as Error, PrefixJobDetail, PrefixJobMetadata, PrefixJobTarget,
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
                header.origin.tenant_id().as_str() == self.webui_tenant_id()
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
        if matches!(
            &header.origin,
            crate::prefix_job_definitions::Origin::Private { .. }
        ) {
            return self
                .inspect_private_prefix_header(pool, header, target)
                .await;
        }
        let chat_scope = header.origin.chat_scope().map_err(|_| unavailable())?;
        let state = match self
            .turn_coordinator
            .get_run_state(GetRunStateRequest {
                scope: chat_scope.clone(),
                run_id: TurnRunId::from_uuid(id),
            })
            .await
        {
            Ok(state) => Some(state),
            Err(brassclaw_turns::TurnError::ScopeNotFound) => None,
            Err(_) => return Err(unavailable()),
        };
        if state.as_ref().is_some_and(|state| {
            state.scope != *chat_scope
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
                    != serde_json::to_value(chat_scope).map_err(|_| unavailable())?
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
        Ok(PrefixJobDetail::Chat {
            thread_id: chat_scope.thread_id.to_string(),
            run_status: state.as_ref().map(|state| state.status),
            metadata: PrefixJobMetadata {
                job_id: header.id.to_string(),
                target,
                operation: header.operation,
                mode: header.mode,
                created_at: header.created_at,
                request_sha256: header.request_sha256,
                program_sha256: header.program_sha256,
                input_sha256: header.input_sha256,
                admission_phase: phase,
                answered_tool_calls: u64::try_from(calls.get::<_, i64>(0))
                    .map_err(|_| unavailable())?,
                pending_tool_calls: u64::try_from(calls.get::<_, i64>(1))
                    .map_err(|_| unavailable())?,
                failure_kind: state
                    .and_then(|state| state.failure)
                    .map(|failure| failure.into_category()),
                internal_receipt: receipt,
                result_ref: reference,
            },
        })
    }
}

impl RebornRuntime {
    async fn inspect_private_prefix_header(
        &self,
        pool: std::sync::Arc<brassclaw_pg::PgPool>,
        header: crate::prefix_job_definitions::Header,
        target: PrefixJobTarget,
    ) -> Result<PrefixJobDetail, Error> {
        use brassclaw_turns::private_work::{PrivateOriginalRef, PrivateWorkStatus};
        use sha2::{Digest, Sha256};
        let unavailable = || Error::Unavailable;
        let context = header
            .origin
            .context(&header.created_by)
            .map_err(|_| unavailable())?;
        let digests = hex::decode(format!(
            "{}{}{}",
            header.request_sha256, header.program_sha256, header.input_sha256
        ))
        .map_err(|_| unavailable())?;
        let original = PrivateOriginalRef::PrefixSourceOnly {
            job_id: header.id,
            definition_checksum: Sha256::digest(digests).into(),
        };
        let store =
            brassclaw_turns::PgTurnStateStore::new(pool.clone(), context.tenant_id.as_str());
        let mut client = pool.get().await.map_err(|_| unavailable())?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| unavailable())?;
        let state = store
            .get_private_work_state_in_transaction(&tx, header.id)
            .await
            .map_err(|_| unavailable())?;
        if state.as_ref().is_some_and(|state| {
            state.request.original() != &original
                || state.request.context() != &context
                || state.request.run_id().as_uuid() != header.id
        }) {
            return Err(unavailable());
        }
        let row = tx
            .query_opt(
                "SELECT origin_kind,private_work_id,private_execution_context,
            private_request_checksum,phase,outcome,result_only_completion
            FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&header.id],
            )
            .await
            .map_err(|_| unavailable())?;
        let mut phase = None;
        let mut receipt = None;
        let mut reference = None;
        let mut failure_kind = None;
        let mut delivery_checksum = None;
        let mut delivery_pending = false;
        if let Some(row) = row {
            let state = state.as_ref().ok_or_else(unavailable)?;
            if row.get::<_, String>(0) != "private_work"
                || row.get::<_, Uuid>(1) != header.id
                || row.get::<_, Value>(2)
                    != serde_json::to_value(&context).map_err(|_| unavailable())?
                || row.get::<_, Vec<u8>>(3).as_slice() != state.request_checksum
                || !row.get::<_, bool>(6)
            {
                return Err(unavailable());
            }
            let actual: String = row.get(4);
            if (actual == "settled") != (state.status == PrivateWorkStatus::Settled) {
                return Err(unavailable());
            }
            if let Some(outcome) = row.get::<_, Option<Value>>(5) {
                crate::pg_monty_admission::validate_outcome(&outcome, true, state.request.run_id())
                    .map_err(|_| unavailable())?;
                if actual != "settled" || state.terminal_checksum.is_none() {
                    return Err(unavailable());
                }
                if outcome["status"] == "completed" {
                    let delivered =
                        crate::pg_monty_admission::PgMontyAdmission::read_private_delivery(
                            &tx, header.id, &context,
                        )
                        .await
                        .map_err(|_| unavailable())?;
                    if let Some(delivered) = delivered {
                        if delivered.original().request().original() != &original
                            || delivered.outcome() != &outcome
                        {
                            return Err(unavailable());
                        }
                        delivery_checksum = Some(hex::encode(delivered.checksum()));
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
                    } else {
                        // A stored terminal precedes actual Factory return.
                        // Missing acknowledgement is visible, never completion.
                        delivery_pending = true;
                    }
                } else {
                    failure_kind = Some(
                        outcome["reason_kind"]
                            .as_str()
                            .ok_or_else(unavailable)?
                            .to_owned(),
                    );
                }
            }
            phase = Some(actual);
        } else if state.as_ref().is_some_and(|state| {
            matches!(
                state.status,
                PrivateWorkStatus::Claimed
                    | PrivateWorkStatus::ReconciliationRequired
                    | PrivateWorkStatus::Settled
            )
        }) {
            return Err(unavailable());
        }
        let calls=tx.query_one("SELECT count(*) FILTER(WHERE phase='answered'),
            count(*) FILTER(WHERE phase='dispatch_intent') FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
            &[&header.id]).await.map_err(|_|unavailable())?;
        let detail = PrefixJobDetail::PrivateWork {
            work_id: header.id.to_string(),
            work_status: state.map(|state| state.status),
            delivery_checksum,
            delivery_pending,
            metadata: PrefixJobMetadata {
                job_id: header.id.to_string(),
                target,
                operation: header.operation,
                mode: header.mode,
                created_at: header.created_at,
                request_sha256: header.request_sha256,
                program_sha256: header.program_sha256,
                input_sha256: header.input_sha256,
                admission_phase: phase,
                answered_tool_calls: u64::try_from(calls.get::<_, i64>(0))
                    .map_err(|_| unavailable())?,
                pending_tool_calls: u64::try_from(calls.get::<_, i64>(1))
                    .map_err(|_| unavailable())?,
                failure_kind,
                internal_receipt: receipt,
                result_ref: reference,
            },
        };
        tx.commit().await.map_err(|_| unavailable())?;
        Ok(detail)
    }
}
