//! References for a still-live native invocation waiting before dispatch.
//! This is runner lifecycle custody, not a checkpoint, Tool permission or proof
//! that authentication was completed. Composition must verify the original
//! native invocation and host-derived preflight evidence before using the port.
use brassclaw_host_api::{AuthDispatchBoundary, InvocationId, RuntimeCredentialAuthRequirement};
use serde::{Deserialize, Serialize};

use crate::{GateRef, TurnError, TurnLeaseToken, TurnRunId, TurnRunnerId, TurnScope};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockNativeAuthRequest {
    pub scope: TurnScope,
    pub run_id: TurnRunId,
    pub runner_id: TurnRunnerId,
    pub lease_token: TurnLeaseToken,
    pub invocation_id: InvocationId,
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
/// native owners. Deserializing this reference grants no dispatch authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativeAuthWait {
    format: NativeAuthWaitFormat,
    invocation_id: InvocationId,
    phase: NativeAuthWaitPhase,
}

impl NativeAuthWait {
    pub(crate) fn new(invocation_id: InvocationId) -> Result<Self, TurnError> {
        native_auth_gate(invocation_id)?;
        Ok(Self {
            format: NativeAuthWaitFormat::V1,
            invocation_id,
            phase: NativeAuthWaitPhase::Waiting,
        })
    }
    pub fn invocation_id(&self) -> InvocationId {
        self.invocation_id
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
            invocation_id: InvocationId,
            phase: NativeAuthWaitPhase,
        }
        let record = Record::deserialize(deserializer)?;
        let mut wait = Self::new(record.invocation_id).map_err(serde::de::Error::custom)?;
        wait.format = record.format;
        wait.phase = record.phase;
        Ok(wait)
    }
}

pub fn native_auth_gate(invocation_id: InvocationId) -> Result<GateRef, TurnError> {
    if invocation_id.as_uuid().is_nil() {
        return Err(TurnError::InvalidRequest {
            reason: "native authentication occurrence is missing".into(),
        });
    }
    GateRef::new(format!("gate:auth:native:{invocation_id}"))
        .map_err(|reason| TurnError::InvalidRequest { reason })
}
