//! Closed terminal record from actual native receipt/proof. Reading these bytes
//! cannot construct SettledInternalCompletion or authorize a retry/Tool call.
use brassclaw_monty_host::{
    process::{TaskAccounting, TaskHandle},
    service::{TaskOutcome, TaskReceipt, UnsubmittedTask},
};
use brassclaw_turns::run_profile::AgentLoopDriverError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn invalid() -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: "private_terminal_original_invalid".into(),
    }
}
fn checksum(bytes: &str) -> [u8; 32] {
    Sha256::digest(bytes.as_bytes()).into()
}
fn present_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Address {
    pub(super) work_id: Uuid,
    pub(super) run_id: Uuid,
    pub(super) claim_generation: u64,
    pub(super) claim_checksum: [u8; 32],
    pub(super) request_checksum: [u8; 32],
    pub(super) definition_checksum: [u8; 32],
    pub(super) admission_ref: [u8; 32],
    pub(super) cancellation_revision: u64,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Root {
    vm_id: Uuid,
    source_checksum: [u8; 32],
    aliases_checksum: [u8; 32],
    workers: u32,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ComputeTime {
    seconds: u64,
    nanoseconds: u32,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Accounting {
    task: TaskHandle,
    effective_revision: u64,
    #[serde(deserialize_with = "present_option")]
    compute_time: Option<ComputeTime>,
    #[serde(deserialize_with = "present_option")]
    failure: Option<String>,
}
impl From<&TaskAccounting> for Accounting {
    fn from(account: &TaskAccounting) -> Self {
        Self {
            task: account.task,
            effective_revision: account.effective_revision,
            compute_time: account.compute_time.map(|duration| ComputeTime {
                seconds: duration.as_secs(),
                nanoseconds: duration.subsec_nanos(),
            }),
            failure: account.failure.clone(),
        }
    }
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum Outcome {
    InternalCompleted { receipt_checksum: [u8; 32] },
    Failed { reason_kind: String },
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Unsubmitted {
    TaskCancelled,
    MontyInputRejected,
    MontyAdmissionBackpressure,
    MontyServiceClosed,
    MontyAdmissionRejected,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "completion_origin",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Completion {
    NativeReceipt {
        root: Root,
        outcome: Outcome,
        #[serde(deserialize_with = "present_option")]
        accounting: Option<Accounting>,
        execution_report_checksum: [u8; 32],
    },
    NeverSubmitted {
        reason: Unsubmitted,
    },
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct InvocationRef {
    pub(super) invocation_id: Uuid,
    pub(super) selection_checksum: [u8; 32],
    pub(super) arguments_checksum: [u8; 32],
    pub(super) answer_checksum: [u8; 32],
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum Format {
    #[serde(rename = "monty-private-terminal/1")]
    V1,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: Format,
    address: Address,
    completion: Completion,
    reconciliation_refs: Vec<InvocationRef>,
}
pub(super) struct Terminal {
    bytes: String,
    checksum: [u8; 32],
    execution_report_bytes: Option<String>,
}
/// Historical data only. It cannot be converted into a service receipt or an
/// execution/settlement handle. Construction authenticates the original bytes.
pub(super) struct HistoricalNative {
    pub(super) address: Address,
    pub(super) task: TaskHandle,
    pub(super) root: serde_json::Value,
    pub(super) accounting: serde_json::Value,
    pub(super) receipt_checksum: [u8; 32],
    pub(super) reconciliation_refs: Vec<InvocationRef>,
}
impl Terminal {
    pub(super) fn read_success(
        bytes: &str,
        expected: [u8; 32],
        report: &str,
    ) -> Result<HistoricalNative, AgentLoopDriverError> {
        Self::verify(bytes, expected, Some(report))?;
        let wire: Wire = serde_json::from_str(bytes).map_err(|_| invalid())?;
        let Completion::NativeReceipt {
            root,
            outcome: Outcome::InternalCompleted { receipt_checksum },
            accounting: Some(accounting),
            ..
        } = wire.completion
        else {
            return Err(invalid());
        };
        let compute = accounting.compute_time.ok_or_else(invalid)?;
        Ok(HistoricalNative {
            address: wire.address,
            task: accounting.task,
            root: serde_json::json!({"format":"monty-root-execution/1", "vm_id":root.vm_id,
                "source_checksum":hex::encode(root.source_checksum),
                "aliases_checksum":hex::encode(root.aliases_checksum), "workers":root.workers}),
            accounting: serde_json::json!({"effective_revision":accounting.effective_revision,
                "compute_time":{"secs":compute.seconds,"nanos":compute.nanoseconds},
                "failure":accounting.failure}),
            receipt_checksum,
            reconciliation_refs: wire.reconciliation_refs,
        })
    }
    pub(super) fn native(
        address: Address,
        receipt: &TaskReceipt,
        report: &serde_json::Value,
        reconciliation_refs: Vec<InvocationRef>,
    ) -> Result<Self, AgentLoopDriverError> {
        if !receipt.withheld.is_empty() {
            return Err(invalid());
        }
        let outcome = match &receipt.outcome {
            TaskOutcome::InternalCompleted { receipt_ref } => Outcome::InternalCompleted {
                receipt_checksum: checksum(receipt_ref),
            },
            TaskOutcome::Failed { reason_kind } => Outcome::Failed {
                reason_kind: reason_kind.clone(),
            },
            TaskOutcome::Completed { .. } => return Err(invalid()),
        };
        let execution_report_bytes = serde_json::to_string(report).map_err(|_| invalid())?;
        let completion = Completion::NativeReceipt {
            root: Root {
                vm_id: receipt.root.vm_id(),
                source_checksum: receipt.root.source_checksum(),
                aliases_checksum: receipt.root.aliases_checksum(),
                workers: receipt.root.workers(),
            },
            outcome,
            accounting: receipt.accounting.as_ref().map(Accounting::from),
            execution_report_checksum: checksum(&execution_report_bytes),
        };
        Self::encode(
            Wire {
                format: Format::V1,
                address,
                completion,
                reconciliation_refs,
            },
            Some(execution_report_bytes),
        )
    }
    pub(super) fn unsubmitted(
        address: Address,
        proof: &UnsubmittedTask,
    ) -> Result<Self, AgentLoopDriverError> {
        let reason = match proof.reason().reason_kind() {
            "task_cancelled" => Unsubmitted::TaskCancelled,
            "monty_input_rejected" => Unsubmitted::MontyInputRejected,
            "monty_admission_backpressure" => Unsubmitted::MontyAdmissionBackpressure,
            "monty_service_closed" => Unsubmitted::MontyServiceClosed,
            "monty_admission_rejected" => Unsubmitted::MontyAdmissionRejected,
            _ => return Err(invalid()),
        };
        Self::encode(
            Wire {
                format: Format::V1,
                address,
                completion: Completion::NeverSubmitted { reason },
                reconciliation_refs: Vec::new(),
            },
            None,
        )
    }
    fn encode(wire: Wire, report: Option<String>) -> Result<Self, AgentLoopDriverError> {
        let bytes = serde_json::to_string(&wire).map_err(|_| invalid())?;
        let digest = checksum(&bytes);
        Self::verify(&bytes, digest, report.as_deref())?;
        Ok(Self {
            bytes,
            checksum: digest,
            execution_report_bytes: report,
        })
    }
    fn verify(
        bytes: &str,
        expected: [u8; 32],
        report: Option<&str>,
    ) -> Result<(), AgentLoopDriverError> {
        if checksum(bytes) != expected {
            return Err(invalid());
        }
        let wire: Wire = serde_json::from_str(bytes).map_err(|_| invalid())?;
        if wire.address.work_id.is_nil()
            || wire.address.run_id.is_nil()
            || wire.address.claim_generation != 1
        {
            return Err(invalid());
        }
        match &wire.completion {
            Completion::NativeReceipt {
                root,
                outcome,
                accounting,
                execution_report_checksum,
            } => {
                if root.vm_id.is_nil()
                    || root.workers == 0
                    || report.map(checksum) != Some(*execution_report_checksum)
                    || (accounting.is_none() && !wire.reconciliation_refs.is_empty())
                {
                    return Err(invalid());
                }
                if let Some(account) = accounting
                    && (account.effective_revision == 0
                        || account
                            .compute_time
                            .as_ref()
                            .is_some_and(|time| time.nanoseconds >= 1_000_000_000))
                {
                    return Err(invalid());
                }
                if matches!(outcome, Outcome::InternalCompleted { .. })
                    && accounting.as_ref().is_none_or(|account| {
                        account.compute_time.is_none() || account.failure.is_some()
                    })
                {
                    return Err(invalid());
                }
            }
            Completion::NeverSubmitted { .. } => {
                if report.is_some() || !wire.reconciliation_refs.is_empty() {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }
    pub(super) fn bytes(&self) -> &str {
        &self.bytes
    }
    pub(super) fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub(super) fn report_bytes(&self) -> Option<&str> {
        self.execution_report_bytes.as_deref()
    }
}
