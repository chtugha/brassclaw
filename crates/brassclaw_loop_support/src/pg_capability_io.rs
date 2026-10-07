//! Durable, immutable values behind an admitted task's capability references.
//!
//! This is storage, not a dispatch ledger: retaining a result does not establish
//! that a timed-out effect was absent or authorize replay after a crash.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_pg::PgPool;
use brassclaw_turns::{
    LoopResultRef,
    run_profile::{
        AgentLoopHostError, AgentLoopHostErrorKind, CapabilityInputRef, LoopRunContext,
        ProviderToolCall,
    },
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    CapabilityResultWrite, LoopCapabilityInputResolver, LoopCapabilityResultWriter,
    ToolResultPayloadSource,
};

/// The owner must retain rows while a task, checkpoint or transcript needs them.
/// There is deliberately no cache eviction, delete-on-error or mutable output.
#[derive(Clone)]
pub struct PgCapabilityIo {
    pool: Arc<PgPool>,
}

#[async_trait]
impl ToolResultPayloadSource for PgCapabilityIo {
    async fn load_tool_result(
        &self,
        context: &LoopRunContext,
        reference: &LoopResultRef,
    ) -> Result<Value, AgentLoopHostError> {
        // Prior eligible transcript results retain their original run ID. The
        // prompt resolver checks eligibility; storage still checks the exact
        // conversation identity and never accepts another conversation's ref.
        let run = reference
            .as_str()
            .strip_prefix("result:")
            .and_then(|value| value.split_once('.'))
            .and_then(|(run, invocation)| {
                Uuid::parse_str(invocation).ok()?;
                brassclaw_turns::TurnRunId::parse(run).ok()
            })
            .ok_or_else(|| {
                AgentLoopHostError::new(
                    AgentLoopHostErrorKind::InvalidInvocation,
                    "tool result reference is invalid",
                )
            })?;
        self.result_output(&context.scope, run, reference).await
    }
}
impl PgCapabilityIo {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    async fn retain(
        &self,
        context: &LoopRunContext,
        reference: &str,
        kind: &str,
        invocation: Option<Uuid>,
        capability: Option<&str>,
        payload: &Value,
    ) -> Result<(), AgentLoopHostError> {
        // A storage/transport bound, independent of token budgeting. Reject the
        // complete value rather than storing a truncated successful result.
        bounded_value(payload)?;
        if serde_json::to_vec(payload)
            .map_err(|_| unavailable())?
            .len()
            > 16 * 1024 * 1024
        {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::BudgetExceeded,
                "capability value exceeds the durable storage limit",
            ));
        }
        let scope = serde_json::to_value(&context.scope).map_err(|_| unavailable())?;
        let run = context.run_id.as_uuid();
        let connection = self.pool.get().await.map_err(|_| unavailable())?;
        // Conflict is never an update. Verify the complete immutable row before
        // acknowledging an idempotent write, including the caller identity.
        connection
            .execute(
                "INSERT INTO brassclaw_loop_capability_values
                    (reference, scope, run_id, kind, invocation_id, capability_id, payload)
                 VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO NOTHING",
                &[
                    &reference,
                    &scope,
                    &run,
                    &kind,
                    &invocation,
                    &capability,
                    payload,
                ],
            )
            .await
            .map_err(|_| unavailable())?;
        let row = connection
            .query_opt(
                "SELECT scope, run_id, kind, invocation_id, capability_id, payload
                 FROM brassclaw_loop_capability_values WHERE reference = $1",
                &[&reference],
            )
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
        if row.get::<_, Value>(0) != scope
            || row.get::<_, Uuid>(1) != run
            || row.get::<_, String>(2) != kind
            || row.get::<_, Option<Uuid>>(3) != invocation
            || row.get::<_, Option<String>>(4).as_deref() != capability
            || row.get::<_, Value>(5) != *payload
        {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::Internal,
                "capability reference conflicts with its retained value",
            ));
        }
        Ok(())
    }

    async fn load(
        &self,
        scope: &brassclaw_turns::TurnScope,
        run: brassclaw_turns::TurnRunId,
        reference: &str,
        kind: &str,
    ) -> Result<Value, AgentLoopHostError> {
        let scope = serde_json::to_value(scope).map_err(|_| unavailable())?;
        self.pool
            .get()
            .await
            .map_err(|_| unavailable())?
            .query_opt(
                "SELECT payload FROM brassclaw_loop_capability_values
                 WHERE reference = $1 AND scope = $2 AND run_id = $3 AND kind = $4",
                &[&reference, &scope, &run.as_uuid(), &kind],
            )
            .await
            .map_err(|_| unavailable())?
            .map(|row| row.get(0))
            .ok_or_else(|| {
                AgentLoopHostError::new(
                    AgentLoopHostErrorKind::InvalidInvocation,
                    "capability reference is unavailable for this task",
                )
            })
    }

    pub async fn result_output(
        &self,
        scope: &brassclaw_turns::TurnScope,
        run: brassclaw_turns::TurnRunId,
        reference: &LoopResultRef,
    ) -> Result<Value, AgentLoopHostError> {
        self.load(scope, run, reference.as_str(), "result").await
    }
}

#[async_trait]
impl LoopCapabilityInputResolver for PgCapabilityIo {
    async fn resolve_capability_input(
        &self,
        context: &LoopRunContext,
        reference: &CapabilityInputRef,
    ) -> Result<Value, AgentLoopHostError> {
        self.load(&context.scope, context.run_id, reference.as_str(), "input")
            .await
    }

    async fn register_provider_tool_call_input(
        &self,
        context: &LoopRunContext,
        call: &ProviderToolCall,
    ) -> Result<CapabilityInputRef, AgentLoopHostError> {
        // Include the complete provider-call identity and bytes. All of this
        // remains typed data; the digest is not a policy/approval fingerprint.
        let bytes = serde_json::to_vec(call).map_err(|_| unavailable())?;
        let digest = brassclaw_host_api::sha256_digest_token(&bytes);
        let reference = CapabilityInputRef::new(format!("input:{}:{digest}", context.run_id))
            .map_err(|_| unavailable())?;
        self.retain(
            context,
            reference.as_str(),
            "input",
            None,
            None,
            &call.arguments,
        )
        .await?;
        Ok(reference)
    }
}

#[async_trait]
impl LoopCapabilityResultWriter for PgCapabilityIo {
    async fn write_capability_result(
        &self,
        write: CapabilityResultWrite<'_>,
    ) -> Result<LoopResultRef, AgentLoopHostError> {
        let reference = LoopResultRef::new(format!(
            "result:{}.{}",
            write.run_context.run_id, write.invocation_id,
        ))
        .map_err(|_| unavailable())?;
        self.retain(
            write.run_context,
            reference.as_str(),
            "result",
            Some(write.invocation_id.as_uuid()),
            Some(write.capability_id.as_str()),
            &write.output,
        )
        .await?;
        Ok(reference)
    }

    async fn delete_capability_result(
        &self,
        _context: &LoopRunContext,
        _reference: &LoopResultRef,
    ) -> Result<(), AgentLoopHostError> {
        Err(AgentLoopHostError::new(
            AgentLoopHostErrorKind::InvalidInvocation,
            "retained capability results require lifecycle-aware reclamation",
        ))
    }
}

fn unavailable() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Unavailable,
        "durable capability value store unavailable",
    )
}

fn bounded_value(value: &Value) -> Result<(), AgentLoopHostError> {
    let mut pending = vec![(value, 0usize)];
    let mut nodes = 1_000_000usize;
    let mut bytes = 16 * 1024 * 1024usize;
    while let Some((value, depth)) = pending.pop() {
        if depth > 64 || nodes == 0 {
            return Err(value_limit());
        }
        nodes -= 1;
        let size = match value {
            Value::String(value) => value.len(),
            Value::Array(values) => {
                if values.len().saturating_add(pending.len()) > nodes {
                    return Err(value_limit());
                }
                pending.extend(values.iter().map(|value| (value, depth + 1)));
                0
            }
            Value::Object(values) => {
                if values.len().saturating_add(pending.len()) > nodes {
                    return Err(value_limit());
                }
                let size = values
                    .keys()
                    .try_fold(0usize, |total, key| total.checked_add(key.len()))
                    .ok_or_else(value_limit)?;
                pending.extend(values.values().map(|value| (value, depth + 1)));
                size
            }
            _ => 0,
        };
        bytes = bytes.checked_sub(size).ok_or_else(value_limit)?;
    }
    Ok(())
}
fn value_limit() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::BudgetExceeded,
        "capability value exceeds the durable storage limit",
    )
}
