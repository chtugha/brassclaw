//! Native model-answer custody for one actual committed selected Tool dispatch.
//! No provider calls, task sequencing, approval inference or effect replay.
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use async_trait::async_trait;
use brassclaw_host_api::InvocationId;
use brassclaw_reborn::monty_model_capability::{
    KOHAI_COMPLETE_CAPABILITY_ID, RecipeModelResponseRecord, RecipeModelResponseStore,
};
use brassclaw_turns::run_profile::{AgentLoopHostError, AgentLoopHostErrorKind, LoopModelResponse};
use serde_json::Value;

use super::{
    PgMontyAdmission,
    invocation::{PgMontyInvocation, digest, encode_json, encoded},
};
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    value_contract::{ContractLimits, strict_json},
};

fn error() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Unavailable,
        "native model response evidence unavailable",
    )
}

/// Owned by the task-bound kernel registration; failed tasks retain that owner.
/// A successful task may release it only after every received response is durable.
pub(crate) struct PgRecipeModelResponses {
    admission: Arc<PgMontyAdmission>,
    prefix_reference: String,
    records: Mutex<HashMap<InvocationId, Arc<Record>>>,
}
impl PgRecipeModelResponses {
    pub(crate) fn new(admission: Arc<PgMontyAdmission>, prefix_reference: String) -> Self {
        Self {
            admission,
            prefix_reference,
            records: Mutex::new(HashMap::new()),
        }
    }
}
struct Record {
    invocation: PgMontyInvocation,
    prefix_reference: String,
    state: tokio::sync::Mutex<State>,
    pending: AtomicBool,
}
enum State {
    Awaiting,
    Received(Box<LoopModelResponse>),
    Persisted,
}
struct Handle(Arc<Record>);

#[async_trait]
impl RecipeModelResponseStore for PgRecipeModelResponses {
    fn has_uncommitted_results(&self) -> bool {
        self.records
            .lock()
            .map(|records| records.values().any(|r| r.pending.load(Ordering::Acquire)))
            .unwrap_or(true)
    }
    async fn reconcile_received_response(
        &self,
        invocation: InvocationId,
    ) -> Result<(), AgentLoopHostError> {
        let record = self
            .records
            .lock()
            .map_err(|_| error())?
            .get(&invocation)
            .cloned()
            .ok_or_else(error)?;
        let mut state = record.state.lock().await;
        match &*state {
            State::Persisted => return Ok(()),
            State::Awaiting => return Err(error()),
            State::Received(_) => {}
        }
        let State::Received(response) = &*state else {
            return Err(error());
        };
        let bytes =
            encode_json(response, record.invocation.data_limits.max_bytes).map_err(|_| error())?;
        record
            .invocation
            .record_native_model_response(&record.prefix_reference, &bytes)
            .await?;
        *state = State::Persisted;
        record.pending.store(false, Ordering::Release);
        Ok(())
    }
    async fn prepare(
        &self,
        invocation: InvocationId,
        arguments: &Value,
        prefix_reference: &str,
    ) -> Result<Box<dyn RecipeModelResponseRecord>, AgentLoopHostError> {
        if invocation.as_uuid().is_nil()
            || prefix_reference.is_empty()
            || prefix_reference != self.prefix_reference
        {
            return Err(error());
        }
        let mut client = self.admission.pool.get().await.map_err(|_| error())?;
        let tx = client.transaction().await.map_err(|_| error())?;
        self.admission
            .preflight_claim(&tx)
            .await
            .map_err(|_| error())?;
        let row = tx.query_opt(
            "SELECT i.invocation_key,i.selection_bytes,i.arguments_bytes,i.phase,
                    i.data_max_depth,i.data_max_nodes,i.data_max_bytes,
                    EXISTS(SELECT 1 FROM brassclaw_monty_model_responses r WHERE r.invocation_id=i.invocation_id)
             FROM brassclaw_monty_tool_invocations i
             WHERE i.invocation_id=$1 AND i.run_id=$2 AND i.admission_key=$3",
            &[&invocation.as_uuid(),&self.admission.attempt.run_id.as_uuid(),&&self.admission.key[..]],
        ).await.map_err(|_| error())?.ok_or_else(error)?;
        let selection =
            strict_json(&row.get::<_, String>(1), REVISION_LIMITS).map_err(|_| error())?;
        let limits = ContractLimits {
            max_depth: usize::try_from(row.get::<_, i64>(4)).map_err(|_| error())?,
            max_nodes: usize::try_from(row.get::<_, i64>(5)).map_err(|_| error())?,
            max_bytes: usize::try_from(row.get::<_, i64>(6)).map_err(|_| error())?,
        };
        if row.get::<_, &str>(3) != "dispatch_intent"
            || row.get::<_, bool>(7)
            || selection["capability_id"] != KOHAI_COMPLETE_CAPABILITY_ID
            || encoded(arguments, limits).map_err(|_| error())? != row.get::<_, String>(2)
        {
            return Err(error());
        }
        let key: [u8; 32] = row.get::<_, Vec<u8>>(0).try_into().map_err(|_| error())?;
        self.admission
            .verify_claim(&tx)
            .await
            .map_err(|_| error())?;
        tx.commit().await.map_err(|_| error())?;
        drop(client);
        let record = Arc::new(Record {
            invocation: PgMontyInvocation {
                admission: self.admission.clone(),
                id: invocation.as_uuid(),
                key,
                data_limits: limits,
            },
            prefix_reference: prefix_reference.to_owned(),
            state: tokio::sync::Mutex::new(State::Awaiting),
            pending: AtomicBool::new(false),
        });
        let mut records = self.records.lock().map_err(|_| error())?;
        if records.contains_key(&invocation) {
            return Err(error());
        }
        records.try_reserve(1).map_err(|_| error())?;
        records.insert(invocation, record.clone());
        Ok(Box::new(Handle(record)))
    }
}

#[async_trait]
impl RecipeModelResponseRecord for Handle {
    async fn retain_response(
        &self,
        response: LoopModelResponse,
    ) -> Result<String, AgentLoopHostError> {
        let mut state = self.0.state.lock().await;
        if !matches!(*state, State::Awaiting) {
            return Err(error());
        }
        // Take custody before any encoding/DB work. A failed or ambiguous write
        // retains these actual bytes' source; it never resets Awaiting/replays.
        *state = State::Received(Box::new(response));
        self.0.pending.store(true, Ordering::Release);
        let State::Received(response) = &*state else {
            return Err(error());
        };
        let bytes =
            encode_json(response, self.0.invocation.data_limits.max_bytes).map_err(|_| error())?;
        self.0
            .invocation
            .record_native_model_response(&self.0.prefix_reference, &bytes)
            .await?;
        *state = State::Persisted;
        self.0.pending.store(false, Ordering::Release);
        Ok(bytes)
    }
}

impl PgMontyInvocation {
    async fn record_native_model_response(
        &self,
        prefix: &str,
        bytes: &str,
    ) -> Result<(), AgentLoopHostError> {
        let mut client = self.admission.pool.get().await.map_err(|_| error())?;
        let tx = client.transaction().await.map_err(|_| error())?;
        // This is evidence for the original effect, including its late answer.
        // A cancelled/stale claim may retain it, never initiate another dispatch.
        let valid = tx
            .query_opt(
                "SELECT 1 FROM brassclaw_monty_tool_invocations
             WHERE invocation_id=$1 AND run_id=$2 AND admission_key=$3 AND invocation_key=$4
               AND selection_bytes::jsonb->>'capability_id'='host.kohai_complete'",
                &[
                    &self.id,
                    &self.admission.attempt.run_id.as_uuid(),
                    &&self.admission.key[..],
                    &&self.key[..],
                ],
            )
            .await
            .map_err(|_| error())?
            .is_some();
        if !valid || bytes.len() > self.data_limits.max_bytes {
            return Err(error());
        }
        tx.execute(
            "INSERT INTO brassclaw_monty_model_responses(invocation_id,format,prefix_reference,response_bytes,response_checksum)
             VALUES($1,'loop-model-response/1',$2,$3,$4) ON CONFLICT(invocation_id) DO NOTHING",
            &[&self.id,&prefix,&bytes,&digest(bytes)],
        ).await.map_err(|_| error())?;
        let row = tx.query_one(
            "SELECT format,prefix_reference,response_bytes FROM brassclaw_monty_model_responses WHERE invocation_id=$1",
            &[&self.id],
        ).await.map_err(|_| error())?;
        if row.get::<_, &str>(0) != "loop-model-response/1"
            || row.get::<_, &str>(1) != prefix
            || row.get::<_, &str>(2) != bytes
        {
            return Err(error());
        }
        tx.commit().await.map_err(|_| error())
    }
}

impl PgMontyAdmission {
    pub(crate) fn model_response_store(
        self: &Arc<Self>,
        prefix_reference: String,
    ) -> Arc<dyn RecipeModelResponseStore> {
        Arc::new(PgRecipeModelResponses::new(self.clone(), prefix_reference))
    }
}
