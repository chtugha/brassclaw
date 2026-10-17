//! Durable dispatch-intent/answer retention for the stop-only candidate path.
//! No latest lookup, activation, permission, retry or automatic effect replay.
use std::{
    io::{self, Write},
    sync::Arc,
};

use brassclaw_engine::memory::{
    retained_instruction::{RetainedRecipeInstruction, RetainedSelectionError},
    retained_tools::RetainedToolProgram,
};
use brassclaw_monty_host::process::PortAnswer;
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::REVISION_LIMITS,
    value_contract::{ContractLimits, validate_data_bounds},
};
use brassclaw_turns::run_profile::AgentLoopDriverError;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;
use uuid::Uuid;

use super::{PgMontyAdmission, failed};

/// Private handle to one committed intent. Its actual answer may be recorded
/// after cancellation/claim loss. That is evidence retention, never dispatch.
/// No Serialize/Debug/Clone: routing keys and arguments remain host-private.
pub(crate) struct PgMontyInvocation {
    admission: Arc<PgMontyAdmission>,
    id: Uuid,
    key: [u8; 32],
    data_limits: ContractLimits,
}
fn reference(value: ComponentRevisionRef) -> Value {
    let checksum: String = value
        .checksum
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    json!({"uuid":value.uuid,"class_code":value.class_code,"version":value.version,"checksum":checksum})
}
fn digest(bytes: &str) -> String {
    format!("{:x}", Sha256::digest(bytes.as_bytes()))
}
// Stream escaping/envelope bytes through the admitted journal bound instead of
// allocating an unlimited String or a second complete copy of the host answer.
struct BoundedJson {
    bytes: Vec<u8>,
    limit: usize,
    capacity_failed: bool,
}
impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let required = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|size| *size <= self.limit);
        let Some(required) = required else {
            self.capacity_failed = true;
            return Err(io::Error::other("invocation data capacity exceeded"));
        };
        if required > self.bytes.capacity() {
            let target = self
                .bytes
                .capacity()
                .checked_mul(2)
                .unwrap_or(self.limit)
                .max(required)
                .min(self.limit);
            if self
                .bytes
                .try_reserve_exact(target - self.bytes.len())
                .is_err()
            {
                self.capacity_failed = true;
                return Err(io::Error::other("invocation data capacity exceeded"));
            }
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn encode_json(
    value: &impl serde::Serialize,
    limit: usize,
) -> Result<String, AgentLoopDriverError> {
    let mut writer = BoundedJson {
        bytes: Vec::new(),
        limit,
        capacity_failed: false,
    };
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(failed(if writer.capacity_failed {
            "monty_invocation_capacity_exceeded"
        } else {
            "monty_invocation_data_invalid"
        }));
    }
    String::from_utf8(writer.bytes).map_err(|_| failed("monty_invocation_data_invalid"))
}
fn encoded(value: &Value, limits: ContractLimits) -> Result<String, AgentLoopDriverError> {
    validate_data_bounds(value, limits)
        .map_err(|_| failed("monty_invocation_capacity_exceeded"))?;
    encode_json(value, limits.max_bytes)
}
fn encoded_answer(
    answer: &PortAnswer,
    limits: ContractLimits,
) -> Result<String, AgentLoopDriverError> {
    // The classified transport wrapper is metadata, not part of the returned
    // value's node/depth graph. Complete encoded bytes still obey the DB bound.
    if let PortAnswer::Return { value } = answer {
        validate_data_bounds(value, limits)
            .map_err(|_| failed("monty_invocation_capacity_exceeded"))?;
    }
    encode_json(answer, limits.max_bytes)
}

impl PgMontyAdmission {
    /// Retain before the first executable feed, including a pure-logic step.
    /// Exact repetition retains the original record; a changed revision,
    /// variant, layout, order or dependency selection cannot replace it.
    /// This is selection evidence, not activation or combination approval.
    pub(crate) async fn retain_recipe_selection(
        &self,
        instruction: &RetainedRecipeInstruction,
    ) -> Result<(), AgentLoopDriverError> {
        let selection = workflow_selection(instruction)?;
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?;
        self.preflight_claim(&tx).await?;
        self.retain_selection_in_transaction(&tx, instruction, &selection)
            .await?;
        self.verify_claim(&tx).await?;
        tx.commit()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))
    }

    async fn retain_selection_in_transaction(
        &self,
        tx: &Transaction<'_>,
        instruction: &RetainedRecipeInstruction,
        selection: &str,
    ) -> Result<(), AgentLoopDriverError> {
        let admission = tx
            .query_opt(
                "SELECT phase FROM brassclaw_monty_task_admissions
             WHERE run_id=$1 AND admission_key=$2 AND claim_checksum=$3 FOR UPDATE",
                &[
                    &self.attempt.run_id.as_uuid(),
                    &&self.key[..],
                    &&self.checksum[..],
                ],
            )
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?
            .ok_or_else(|| failed("monty_invocation_admission_fenced"))?;
        if admission.get::<_, &str>(0) != "started" {
            return Err(failed("monty_invocation_admission_not_started"));
        }
        tx.execute(
            "INSERT INTO brassclaw_monty_recipe_selections
             (run_id,recipe_id,selection_bytes,selection_checksum)
             VALUES ($1,$2,$3,$4) ON CONFLICT (run_id,recipe_id) DO NOTHING",
            &[
                &self.attempt.run_id.as_uuid(),
                &instruction.recipe().uuid,
                &selection,
                &digest(selection),
            ],
        )
        .await
        .map_err(|_| failed("monty_invocation_database_failed"))?;
        let selected = tx
            .query_one(
                "SELECT selection_bytes,selection_checksum FROM brassclaw_monty_recipe_selections
                 WHERE run_id=$1 AND recipe_id=$2",
                &[&self.attempt.run_id.as_uuid(), &instruction.recipe().uuid],
            )
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?;
        if selected.get::<_, &str>(0) != selection
            || selected.get::<_, &str>(1) != digest(selection)
        {
            return Err(failed("monty_recipe_selection_conflict"));
        }
        Ok(())
    }

    /// Commit before the actual kernel/implementation call. Complete selection
    /// references and association bytes come from the retained IBS program;
    /// this does not establish their missing approval/artifact provenance.
    /// A repeated flat Recipe/step/run is an explicit reconciliation error, including
    /// when a replacement worker has a new claim. Only initial stop dispatch is
    /// supported; guarded repeated steps/retries need their own durable address.
    pub(crate) async fn begin_tool_invocation(
        self: &Arc<Self>,
        program: &RetainedToolProgram,
        step_id: &str,
        arguments: &Value,
        data_limits: ContractLimits,
    ) -> Result<PgMontyInvocation, AgentLoopDriverError> {
        let binding = program
            .bindings()
            .get(step_id)
            .ok_or_else(|| failed("monty_invocation_selection_invalid"))?;
        if step_id.is_empty()
            || step_id.len() > 64
            || binding.association().failure().max_attempts() != 1
            || binding.association().failure().action()
                != brassclaw_skills::association_contract::FailureAction::Stop
            || !arguments.is_object()
        {
            return Err(failed("monty_invocation_selection_invalid"));
        }
        let instruction = program.inputs().instruction();
        let workflow = workflow_selection(instruction)?;
        let selection = Self::invocation_selection(program, step_id)?;
        let arguments = encoded(arguments, data_limits)?;
        let depth = i64::try_from(data_limits.max_depth)
            .map_err(|_| failed("monty_invocation_capacity_exceeded"))?;
        let nodes = i64::try_from(data_limits.max_nodes)
            .map_err(|_| failed("monty_invocation_capacity_exceeded"))?;
        let bytes = i64::try_from(data_limits.max_bytes)
            .map_err(|_| failed("monty_invocation_capacity_exceeded"))?;
        let id = Uuid::new_v4();
        let key: [u8; 32] = Sha256::digest(Uuid::new_v4().as_bytes()).into();
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?;
        self.preflight_claim(&tx).await?;
        self.retain_selection_in_transaction(&tx, instruction, &workflow)
            .await?;
        let inserted = tx.execute(
            "INSERT INTO brassclaw_monty_tool_invocations
             (invocation_id,run_id,recipe_id,step_id,admission_key,invocation_key,
              selection_bytes,selection_checksum,arguments_bytes,arguments_checksum,attempt_count,phase,
              data_max_depth,data_max_nodes,data_max_bytes)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,1,'dispatch_intent',$11,$12,$13)
             ON CONFLICT (run_id,recipe_id,step_id) DO NOTHING",
            &[&id,&self.attempt.run_id.as_uuid(),&instruction.recipe().uuid,&step_id,&&self.key[..],&&key[..],
                &selection,&digest(&selection),&arguments,&digest(&arguments),&depth,&nodes,&bytes],
        ).await.map_err(|_| failed("monty_invocation_database_failed"))?;
        if inserted != 1 {
            return Err(failed("monty_invocation_replay_requires_reconciliation"));
        }
        // This final locked claim check is the commit/dispatch fence. Waiting
        // for an effect-free intent insert must not block turn cancellation.
        self.verify_claim(&tx).await?;
        tx.commit()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?;
        Ok(PgMontyInvocation {
            admission: self.clone(),
            id,
            key,
            data_limits,
        })
    }
}

impl PgMontyAdmission {
    /// Exact occurrence identity shared by the journal producer and trusted readers.
    /// It includes the usage association; the separate workflow selection retains
    /// the complete typed input layout and dependency graph.
    pub(crate) fn invocation_selection(
        program: &RetainedToolProgram,
        step_id: &str,
    ) -> Result<String, AgentLoopDriverError> {
        let binding = program
            .bindings()
            .get(step_id)
            .ok_or_else(|| failed("monty_invocation_selection_invalid"))?;
        let instruction = program.inputs().instruction();
        let references: Vec<_> = instruction
            .snapshot()
            .revisions()
            .values()
            .map(|revision| reference(revision.reference()))
            .collect();
        let mut selection = json!({
            "format":"monty-stop-invocation-selection/1",
            "recipe":reference(instruction.recipe()), "variant_key":instruction.variant().variant_key,
            "step_link":instruction.variant().step_link, "step_order":instruction.ordered().step_order(),
            "step_id":step_id, "components":references,
            "association":binding.association().exact_bytes(),
            "capability_id":binding.capability_id(), "tool":reference(binding.tool()),
            "tool_skill":reference(binding.tool_skill()), "skill":reference(binding.skill()), "python":reference(binding.python()),
        });
        if !instruction.association_approvals().is_empty() {
            selection["association_approvals"] = json!(instruction.association_approvals());
        }
        encoded(&selection, REVISION_LIMITS)
    }
}

fn workflow_selection(
    instruction: &RetainedRecipeInstruction,
) -> Result<String, AgentLoopDriverError> {
    instruction
        .retained_selection()
        .map(|selection| selection.exact_bytes().to_owned())
        .map_err(|error| match error {
            RetainedSelectionError::Invalid => failed("monty_invocation_selection_invalid"),
            RetainedSelectionError::Capacity => failed("monty_invocation_capacity_exceeded"),
        })
}
impl PgMontyInvocation {
    /// Persist the actual host answer before child resume/output validation.
    /// Return/domain/terminal distinctions survive; an error does not prove
    /// that an external effect did not occur. Missing/oversized/failed answer
    /// persistence leaves the intent uncertain and never authorizes replay.
    pub(crate) async fn record_answer(
        &self,
        answer: &PortAnswer,
    ) -> Result<(), AgentLoopDriverError> {
        match answer {
            PortAnswer::Return { .. } => {}
            PortAnswer::DomainError { reason_kind } | PortAnswer::TerminalError { reason_kind }
                if reason_kind.is_empty()
                    || reason_kind.len() > 64
                    || !reason_kind
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'_') =>
            {
                return Err(failed("monty_invocation_data_invalid"));
            }
            _ => {}
        }
        let answer = encoded_answer(answer, self.data_limits)?;
        let mut client = self
            .admission
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))?;
        // The retained admission and invocation keys identify the original
        // effect. A stale claim may record its own late result, never a new call.
        let row = tx.query_opt(
            "SELECT phase,answer_bytes FROM brassclaw_monty_tool_invocations
             WHERE invocation_id=$1 AND run_id=$2 AND admission_key=$3 AND invocation_key=$4 FOR UPDATE",
            &[&self.id,&self.admission.attempt.run_id.as_uuid(),&&self.admission.key[..],&&self.key[..]],
        ).await.map_err(|_| failed("monty_invocation_database_failed"))?
            .ok_or_else(|| failed("monty_invocation_fenced"))?;
        match row.get::<_, &str>(0) {
            "answered" if row.get::<_, Option<String>>(1).as_deref() == Some(answer.as_str()) => {}
            "answered" => return Err(failed("monty_invocation_answer_conflict")),
            "dispatch_intent" => {
                let changed = tx.execute(
                    "UPDATE brassclaw_monty_tool_invocations
                     SET phase='answered',answer_bytes=$2,answer_checksum=$3,answered_at=clock_timestamp()
                     WHERE invocation_id=$1 AND phase='dispatch_intent'",
                    &[&self.id,&answer,&digest(&answer)],
                ).await.map_err(|_| failed("monty_invocation_database_failed"))?;
                if changed != 1 {
                    return Err(failed("monty_invocation_fenced"));
                }
            }
            _ => return Err(failed("monty_invocation_fenced")),
        }
        tx.commit()
            .await
            .map_err(|_| failed("monty_invocation_database_failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invocation_encoding_bounds_value_separately_from_answer_envelope() {
        let limits = ContractLimits {
            max_depth: 1,
            max_nodes: 2,
            max_bytes: 1024,
        };
        let answer = PortAnswer::Return {
            value: json!([null]),
        };
        let encoded = encoded_answer(&answer, limits).unwrap();
        assert_eq!(encoded, serde_json::to_string(&answer).unwrap());
        assert!(
            encoded_answer(
                &PortAnswer::Return {
                    value: json!([null, null])
                },
                limits
            )
            .is_err()
        );
        assert!(
            encoded_answer(
                &PortAnswer::Return {
                    value: json!([[null]])
                },
                limits
            )
            .is_err()
        );
        let exact = ContractLimits {
            max_bytes: encoded.len(),
            ..limits
        };
        assert_eq!(encoded_answer(&answer, exact).unwrap(), encoded);
        assert!(
            encoded_answer(
                &answer,
                ContractLimits {
                    max_bytes: exact.max_bytes - 1,
                    ..limits
                }
            )
            .is_err()
        );
        // Escaped wire bytes, not just the unescaped payload, obey the ceiling.
        let escaped = json!("\u{0000}".repeat(20));
        assert!(
            encoded_answer(
                &PortAnswer::Return { value: escaped },
                ContractLimits {
                    max_bytes: 64,
                    ..limits
                }
            )
            .is_err()
        );
    }
}
