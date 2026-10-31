//! References for a still-live native invocation waiting before dispatch.
//! This is runner lifecycle custody, not a checkpoint, Tool permission or proof
//! that authentication was completed. Composition must verify the original
//! native invocation and host-derived preflight evidence before using the port.
use brassclaw_host_api::{AuthDispatchBoundary, RuntimeCredentialAuthRequirement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{GateRef, TurnError, TurnLeaseToken, TurnRunId, TurnRunnerId, TurnScope};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockNativeAuthRequest {
    pub scope: TurnScope,
    pub run_id: TurnRunId,
    pub runner_id: TurnRunnerId,
    pub lease_token: TurnLeaseToken,
    /// Opaque original occurrence supplied by the native owner; not a Tool grant.
    pub gate_ref: GateRef,
    pub boundary: AuthDispatchBoundary,
    pub credential_requirements: Vec<RuntimeCredentialAuthRequirement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum NativeAuthWaitPhase {
    Waiting,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum NativeAuthWaitFormat {
    #[serde(rename = "native-auth-wait/1")]
    V1,
}

/// Persist only the original occurrence and lifecycle phase. Actual inputs,
/// credentials, selection, flow evidence and effects stay with their existing
/// native owners. The private UUID preserves the existing /1 storage bytes;
/// public turn APIs exchange only the canonical GateRef, not a host identifier.
/// Deserializing this reference grants no dispatch authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativeAuthWait {
    format: NativeAuthWaitFormat,
    invocation_id: Uuid,
    phase: NativeAuthWaitPhase,
}

impl NativeAuthWait {
    pub(crate) fn new(gate_ref: &GateRef) -> Result<Self, TurnError> {
        let invocation_id = gate_ref
            .as_str()
            .strip_prefix("gate:auth:native:")
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| TurnError::InvalidRequest {
                reason: "native authentication occurrence is missing".into(),
            })?;
        if native_auth_gate(invocation_id)?.as_str() != gate_ref.as_str() {
            return Err(TurnError::InvalidRequest {
                reason: "native authentication occurrence reference is not canonical".into(),
            });
        }
        Ok(Self {
            format: NativeAuthWaitFormat::V1,
            invocation_id,
            phase: NativeAuthWaitPhase::Waiting,
        })
    }
    pub fn is_waiting(&self) -> bool {
        self.phase == NativeAuthWaitPhase::Waiting
    }
    pub fn gate_ref(&self) -> Result<GateRef, TurnError> {
        native_auth_gate(self.invocation_id)
    }
    pub(crate) fn resolve(&mut self) {
        self.phase = NativeAuthWaitPhase::Resolved;
    }
}
impl<'de> Deserialize<'de> for NativeAuthWait {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Record {
            format: NativeAuthWaitFormat,
            invocation_id: Uuid,
            phase: NativeAuthWaitPhase,
        }
        let record = Record::deserialize(deserializer)?;
        native_auth_gate(record.invocation_id).map_err(serde::de::Error::custom)?;
        Ok(Self {
            format: record.format,
            invocation_id: record.invocation_id,
            phase: record.phase,
        })
    }
}

fn native_auth_gate(invocation_id: Uuid) -> Result<GateRef, TurnError> {
    if invocation_id.is_nil() {
        return Err(TurnError::InvalidRequest {
            reason: "native authentication occurrence is missing".into(),
        });
    }
    GateRef::new(format!("gate:auth:native:{invocation_id}"))
        .map_err(|reason| TurnError::InvalidRequest { reason })
}
