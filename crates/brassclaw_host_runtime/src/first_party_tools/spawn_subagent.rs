use crate::{
    FirstPartyCapabilityError, FirstPartyCapabilityRequest, FirstPartyCapabilityResult,
    RuntimeChildTaskOutcome, RuntimeChildTaskRequest, RuntimeChildWorkflowRef,
};
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode};
use brassclaw_host_api::{ResourceUsage, RuntimeDispatchErrorKind};

pub const SPAWN_SUBAGENT_CAPABILITY_ID: &str = "builtin.spawn_subagent";

pub(crate) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    super::first_party_capability_manifest(
        SPAWN_SUBAGENT_CAPABILITY_ID,
        "Admit and observe one child workflow through the global orchestrator",
        vec![EffectKind::DispatchCapability],
        PermissionMode::Ask,
        super::resource_profile(),
    )
}

pub(crate) async fn dispatch_child(
    request: FirstPartyCapabilityRequest,
) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
    let invalid = || FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::InputEncode);
    let service = request.services.child_tasks.ok_or_else(|| {
        FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::UnsupportedRunner)
    })?;
    let start = std::time::Instant::now();
    let mut input = request.input;
    let object = input.as_object_mut().ok_or_else(invalid)?;
    if object.len() != 2 {
        return Err(invalid());
    }
    let workflow = object.remove("workflow").ok_or_else(invalid)?;
    let workflow = workflow.as_object().ok_or_else(invalid)?;
    if workflow.len() != 2 {
        return Err(invalid());
    }
    let recipe = workflow
        .get("recipe_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .ok_or_else(invalid)?;
    let variant = workflow
        .get("variant")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(invalid)?;
    let child = RuntimeChildTaskRequest::new(
        RuntimeChildWorkflowRef::new(recipe, variant.to_owned()).map_err(|_| invalid())?,
        object.remove("inputs").ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())?;
    let outcome = service
        .delegate(&request.scope, child)
        .await
        .map_err(|error| {
            FirstPartyCapabilityError::with_safe_summary(
                RuntimeDispatchErrorKind::OperationFailed,
                error.to_string(),
            )
        })?;
    let output = match outcome {
        RuntimeChildTaskOutcome::Completed(reply) => serde_json::json!({
            "status":"completed", "child_run_id":reply.child_run_id,
            "thread_id":reply.thread_id, "reply_ref":reply.reply_ref, "content":reply.content.as_ref(),
        }),
        RuntimeChildTaskOutcome::Failed {
            child_run_id,
            thread_id,
            failure,
        } => serde_json::json!({
            "status":"failed", "child_run_id":child_run_id, "thread_id":thread_id, "failure":failure,
        }),
        RuntimeChildTaskOutcome::Cancelled {
            child_run_id,
            thread_id,
        } => serde_json::json!({
            "status":"cancelled", "child_run_id":child_run_id, "thread_id":thread_id,
        }),
        RuntimeChildTaskOutcome::RecoveryRequired {
            child_run_id,
            thread_id,
        } => serde_json::json!({
            "status":"recovery_required", "child_run_id":child_run_id, "thread_id":thread_id,
        }),
    };
    let output_bytes = super::bounded_output_bytes(&output, isize::MAX as u64)?;
    Ok(FirstPartyCapabilityResult::new(
        output,
        ResourceUsage {
            wall_clock_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
            output_bytes,
            ..ResourceUsage::default()
        },
    ))
}

pub(crate) fn dispatch() -> serde_json::Value {
    serde_json::json!({
        "authorized": true,
    })
}
