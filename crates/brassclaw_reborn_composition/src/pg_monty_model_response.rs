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
    wire: Observation<Value>,
    provider: Observation<Value>,
    response: Observation<Box<LoopModelResponse>>,
}

/// Separate immutable stages: original HTTP bytes, adapter DTO and validated
/// LoopModelResponse. Each may need persistence after cancellation/failure.
struct Observation<T> {
    persisted: tokio::sync::Mutex<bool>,
    pending: AtomicBool,
    observed: AtomicBool,
    value: Mutex<Option<T>>,
}
impl<T> Observation<T> {
    fn new() -> Self {
        Self {
            persisted: tokio::sync::Mutex::new(false),
            pending: AtomicBool::new(false),
            observed: AtomicBool::new(false),
            value: Mutex::new(None),
        }
    }
    fn receive(&self, value: T) -> Result<(), AgentLoopHostError> {
        if self
            .observed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(error());
        }
        self.pending.store(true, Ordering::Release);
        *self.value.lock().map_err(|_| error())? = Some(value);
        Ok(())
    }
}
impl<T: serde::Serialize + Send + Sync> Observation<T> {
    async fn persist(
        &self,
        record: &Record,
        kind: EvidenceKind,
    ) -> Result<Option<String>, AgentLoopHostError> {
        let mut persisted = self.persisted.lock().await;
        if *persisted || !self.observed.load(Ordering::Acquire) {
            return Ok(None);
        }
        let bytes = {
            let value = self.value.lock().map_err(|_| error())?;
            encode_json(
                value.as_ref().ok_or_else(error)?,
                record.invocation.data_limits.max_bytes,
            )
            .map_err(|_| error())?
        };
        record
            .invocation
            .record_model_evidence(kind, &record.prefix_reference, &bytes)
            .await?;
        // Release potentially large completed data after the immutable durable
        // write; retain only the observation fence and persisted state.
        let value = self.value.lock().map_err(|_| error())?.take();
        drop(value);
        self.pending.store(false, Ordering::Release);
        *persisted = true;
        Ok(Some(bytes))
    }
}
struct Handle(Arc<Record>);
#[derive(Clone, Copy)]
enum EvidenceKind {
    Wire,
    Provider,
    Loop,
}
impl EvidenceKind {
    fn table(self) -> &'static str {
        match self {
            Self::Wire => "brassclaw_monty_provider_wire_observations",
            Self::Provider => "brassclaw_monty_provider_observations",
            Self::Loop => "brassclaw_monty_model_responses",
        }
    }
    fn format(self) -> &'static str {
        match self {
            Self::Wire => "provider-wire-observation/1",
            Self::Provider => "provider-model-observation/1",
            Self::Loop => "loop-model-response/1",
        }
    }
}

#[async_trait]
impl RecipeModelResponseStore for PgRecipeModelResponses {
    fn has_uncommitted_results(&self) -> bool {
        self.records
            .lock()
            .map(|records| {
                records.values().any(|r| {
                    r.wire.pending.load(Ordering::Acquire)
                        || r.provider.pending.load(Ordering::Acquire)
                        || r.response.pending.load(Ordering::Acquire)
                })
            })
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
        record.wire.persist(&record, EvidenceKind::Wire).await?;
        record
            .provider
            .persist(&record, EvidenceKind::Provider)
            .await?;
        record.response.persist(&record, EvidenceKind::Loop).await?;
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
                    (EXISTS(SELECT 1 FROM brassclaw_monty_provider_wire_observations r WHERE r.invocation_id=i.invocation_id)
                     OR EXISTS(SELECT 1 FROM brassclaw_monty_model_responses r WHERE r.invocation_id=i.invocation_id)
                     OR EXISTS(SELECT 1 FROM brassclaw_monty_provider_observations r WHERE r.invocation_id=i.invocation_id))
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
            wire: Observation::new(),
            provider: Observation::new(),
            response: Observation::new(),
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
    fn receive_provider_wire_observation(&self, response: Value) -> Result<(), AgentLoopHostError> {
        if response["format"] != "provider-wire-observation/1"
            || response["provider_id"].as_str().is_none_or(str::is_empty)
            || response["model_id"].as_str().is_none_or(str::is_empty)
            || response["body_encoding"] != "base64"
            || !response["body"].is_string()
            || response["status"]
                .as_u64()
                .is_none_or(|status| !(100..=599).contains(&status))
        {
            return Err(error());
        }
        self.0.wire.receive(response)
    }
    async fn persist_provider_wire_observation(&self) -> Result<(), AgentLoopHostError> {
        self.0
            .wire
            .persist(&self.0, EvidenceKind::Wire)
            .await?
            .ok_or_else(error)
            .map(|_| ())
    }

    fn receive_provider_observation(&self, response: Value) -> Result<(), AgentLoopHostError> {
        // This is a host-private adapter envelope. It cannot be supplied by
        // Python; basic envelope checks also reject accidental wrong-stage data.
        if response["format"] != "provider-model-observation/1"
            || response["provider_id"].as_str().is_none_or(str::is_empty)
            || response["model_id"].as_str().is_none_or(str::is_empty)
            || !response["response"].is_object()
        {
            return Err(error());
        }
        self.0.provider.receive(response)
    }
    async fn persist_provider_observation(&self) -> Result<(), AgentLoopHostError> {
        self.0
            .provider
            .persist(&self.0, EvidenceKind::Provider)
            .await?
            .ok_or_else(error)
            .map(|_| ())
    }
    fn receive_response(&self, response: LoopModelResponse) -> Result<(), AgentLoopHostError> {
        if !self.0.provider.observed.load(Ordering::Acquire)
            || self.0.provider.pending.load(Ordering::Acquire)
        {
            return Err(error());
        }
        self.0.response.receive(Box::new(response))
    }
    async fn persist_response(&self) -> Result<String, AgentLoopHostError> {
        self.0
            .response
            .persist(&self.0, EvidenceKind::Loop)
            .await?
            .ok_or_else(error)
    }
}

impl PgMontyInvocation {
    async fn record_model_evidence(
        &self,
        kind: EvidenceKind,
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
        // Only enum-owned table names enter SQL; all runtime values are bound.
        tx.execute(
            &format!("INSERT INTO {}(invocation_id,format,prefix_reference,response_bytes,response_checksum)
             VALUES($1,$2,$3,$4,$5) ON CONFLICT(invocation_id) DO NOTHING",kind.table()),
            &[&self.id,&kind.format(),&prefix,&bytes,&digest(bytes)],
        ).await.map_err(|_| error())?;
        let row = tx
            .query_one(
                &format!(
                    "SELECT format,prefix_reference,response_bytes FROM {} WHERE invocation_id=$1",
                    kind.table()
                ),
                &[&self.id],
            )
            .await
            .map_err(|_| error())?;
        if row.get::<_, &str>(0) != kind.format()
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
