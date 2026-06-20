use std::sync::Arc;

use brassclaw_common::AppEvent;
use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use chrono::Utc;
use serde_json::{Value, json};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::channels::IncomingMessage;
use crate::context::{ContextManager, JobState};
use crate::db::Database;
use crate::orchestrator::job_manager::ContainerJobManager;
use crate::ownership::Owned;
use crate::secrets::SecretsStore;

pub const PROVIDER_ID: &str = "builtin";
pub const CREATE_JOB_CAPABILITY_ID: &str = "builtin.create_job";
pub const CANCEL_JOB_CAPABILITY_ID: &str = "builtin.cancel_job";
pub const LIST_JOBS_CAPABILITY_ID: &str = "builtin.list_jobs";
pub const JOB_STATUS_CAPABILITY_ID: &str = "builtin.job_status";
pub const JOB_EVENTS_CAPABILITY_ID: &str = "builtin.job_events";
pub const JOB_PROMPT_CAPABILITY_ID: &str = "builtin.job_prompt";

const DEFAULT_OUTPUT_BYTES: u64 = 4 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 5_000;
const MAX_WALL_CLOCK_MS: u64 = 60_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct JobsCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl JobsCapabilityError {
    fn input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: true,
        }
    }

    fn operation(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: false,
        }
    }
}

pub type SchedulerSlot = Arc<RwLock<Option<Arc<crate::agent::Scheduler>>>>;
pub type PromptQueue = Arc<
    tokio::sync::Mutex<
        std::collections::HashMap<
            Uuid,
            std::collections::VecDeque<crate::orchestrator::api::PendingPrompt>,
        >,
    >,
>;

pub struct JobsContext {
    pub context_manager: Arc<ContextManager>,
    pub scheduler_slot: Option<SchedulerSlot>,
    pub job_manager: Option<Arc<ContainerJobManager>>,
    pub store: Option<Arc<dyn Database>>,
    pub event_tx: Option<tokio::sync::broadcast::Sender<(Uuid, String, AppEvent)>>,
    pub inject_tx: Option<tokio::sync::mpsc::Sender<IncomingMessage>>,
    pub secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,
    pub prompt_queue: Option<PromptQueue>,
    pub user_id: String,
    pub metadata: Value,
}

fn resource_profile() -> Option<ResourceProfile> {
    Some(ResourceProfile {
        default_estimate: ResourceEstimate {
            wall_clock_ms: Some(DEFAULT_WALL_CLOCK_MS),
            output_bytes: Some(DEFAULT_OUTPUT_BYTES),
            ..ResourceEstimate::default()
        },
        hard_ceiling: Some(ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: Some(MAX_WALL_CLOCK_MS),
            max_output_bytes: Some(MAX_OUTPUT_BYTES),
            sandbox: None,
        }),
    })
}

fn make_descriptor(
    id: &str,
    description: &str,
    effects: Vec<EffectKind>,
    parameters_schema: Value,
    default_permission: PermissionMode,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("valid capability id"),
        provider: ExtensionId::new(PROVIDER_ID).expect("valid provider id"),
        runtime: RuntimeKind::FirstParty,
        trust_ceiling: TrustClass::Sandbox,
        description: description.to_string(),
        parameters_schema,
        effects,
        default_permission,
        runtime_credentials: Vec::new(),
        resource_profile: resource_profile(),
    }
}

pub fn create_job_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        CREATE_JOB_CAPABILITY_ID,
        "Create and schedule a new job for the agent to work on.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "A short title for the job"
                },
                "description": {
                    "type": "string",
                    "description": "Full description of what needs to be done"
                }
            },
            "required": ["title", "description"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn cancel_job_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        CANCEL_JOB_CAPABILITY_ID,
        "Cancel a running or pending job. The job will be marked as cancelled and stopped.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "string",
                    "description": "The job ID (full UUID or short prefix, e.g. 'f2854dd8')"
                }
            },
            "required": ["job_id"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn list_jobs_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        LIST_JOBS_CAPABILITY_ID,
        "List all jobs or filter by status. Shows job IDs, titles, and current status.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "description": "Filter by status: 'active', 'completed', 'failed', 'all' (default: 'all')",
                    "enum": ["active", "completed", "failed", "all"]
                }
            },
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn job_status_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        JOB_STATUS_CAPABILITY_ID,
        "Check the status and details of a specific job by its ID.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "string",
                    "description": "The job ID (full UUID or short prefix, e.g. 'f2854dd8')"
                }
            },
            "required": ["job_id"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn job_events_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        JOB_EVENTS_CAPABILITY_ID,
        "Read the event log for a sandbox job. Shows messages, tool calls, results, \
         and status changes from the container.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "string",
                    "description": "The job ID (full UUID or short prefix, e.g. 'f2854dd8')"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of events to return (default 50, most recent)"
                }
            },
            "required": ["job_id"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn job_prompt_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        JOB_PROMPT_CAPABILITY_ID,
        "Send a follow-up prompt to a running sandbox job. The prompt is queued and \
         delivered on the next poll cycle.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "string",
                    "description": "The job ID (full UUID or short prefix, e.g. 'f2854dd8')"
                },
                "content": {
                    "type": "string",
                    "description": "The follow-up prompt text to send"
                },
                "done": {
                    "type": "boolean",
                    "description": "If true, signals the sub-agent that no more prompts are coming. Default false.",
                    "default": false
                }
            },
            "required": ["job_id", "content"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        create_job_descriptor(),
        cancel_job_descriptor(),
        list_jobs_descriptor(),
        job_status_descriptor(),
        job_events_descriptor(),
        job_prompt_descriptor(),
    ]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, JobsCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| JobsCapabilityError::input(format!("missing required parameter: {key}")))
}

async fn resolve_job_id(
    input: &str,
    context_manager: &ContextManager,
) -> Result<Uuid, JobsCapabilityError> {
    if let Ok(id) = Uuid::parse_str(input) {
        return Ok(id);
    }
    if input.len() < 4 {
        return Err(JobsCapabilityError::input(
            "job ID prefix must be at least 4 hex characters".to_string(),
        ));
    }
    let input_lower = input.to_lowercase();
    let all_ids = context_manager.all_jobs().await;
    let matches: Vec<Uuid> = all_ids
        .into_iter()
        .filter(|id| {
            let hex = id.to_string().replace('-', "");
            hex.starts_with(&input_lower)
        })
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(JobsCapabilityError::input(format!(
            "no job found matching prefix '{}'",
            input
        ))),
        n => Err(JobsCapabilityError::input(format!(
            "ambiguous prefix '{}' matches {} jobs, provide more characters",
            input, n
        ))),
    }
}

pub async fn execute_create_job(
    params: &Value,
    ctx: &JobsContext,
) -> Result<Value, JobsCapabilityError> {
    let title = require_str(params, "title")?;
    let description = require_str(params, "description")?;

    if let Some(ref slot) = ctx.scheduler_slot
        && let Some(ref scheduler) = *slot.read().await
    {
        return match scheduler
            .dispatch_job(&ctx.user_id, title, description, None)
            .await
        {
            Ok(job_id) => Ok(json!({
                "job_id": job_id.to_string(),
                "title": title,
                "status": "in_progress",
                "message": format!("Created and scheduled job '{}'", title)
            })),
            Err(e) => Ok(json!({
                "error": e.to_string()
            })),
        };
    }

    match ctx
        .context_manager
        .create_job_for_user(&ctx.user_id, title, description)
        .await
    {
        Ok(job_id) => Ok(json!({
            "job_id": job_id.to_string(),
            "title": title,
            "status": "pending",
            "message": format!("Created job '{}' (not scheduled — scheduler unavailable)", title)
        })),
        Err(e) => Err(JobsCapabilityError::operation(format!(
            "Failed to create job: {}",
            e
        ))),
    }
}

pub async fn execute_cancel_job(
    params: &Value,
    ctx: &JobsContext,
) -> Result<Value, JobsCapabilityError> {
    let job_id_str = require_str(params, "job_id")?;
    let job_id = resolve_job_id(job_id_str, &ctx.context_manager).await?;
    let requester_id = ctx.user_id.clone();

    match ctx
        .context_manager
        .update_context(job_id, |job_ctx| {
            if !job_ctx.is_owned_by(&requester_id) {
                return Err("Job not found".to_string());
            }
            job_ctx.transition_to(JobState::Cancelled, Some("Cancelled by user".to_string()))
        })
        .await
    {
        Ok(Ok(())) => {
            if let Some(ref jm) = ctx.job_manager
                && let Err(e) = jm.stop_job(job_id).await
            {
                tracing::warn!(
                    job_id = %job_id,
                    "Failed to stop container during cancellation: {}", e
                );
            }

            if let Some(ref store) = ctx.store {
                let store = store.clone();
                tokio::spawn(async move {
                    if let Err(e) = store
                        .update_sandbox_job_status(
                            job_id,
                            "failed",
                            Some(false),
                            Some("Cancelled by user"),
                            None,
                            Some(Utc::now()),
                        )
                        .await
                    {
                        tracing::warn!(
                            job_id = %job_id,
                            "Failed to update sandbox job status on cancel: {}", e
                        );
                    }
                });
            }

            Ok(json!({
                "job_id": job_id.to_string(),
                "status": "cancelled",
                "message": "Job cancelled successfully"
            }))
        }
        Ok(Err(reason)) => Ok(json!({
            "error": format!("Cannot cancel job: {}", reason)
        })),
        Err(e) => Ok(json!({
            "error": format!("Job not found: {}", e)
        })),
    }
}

pub async fn execute_list_jobs(
    params: &Value,
    ctx: &JobsContext,
) -> Result<Value, JobsCapabilityError> {
    let filter = params
        .get("filter")
        .and_then(|v| v.as_str())
        .unwrap_or("all");

    let job_ids = match filter {
        "active" => ctx.context_manager.active_jobs_for(&ctx.user_id).await,
        _ => ctx.context_manager.all_jobs_for(&ctx.user_id).await,
    };

    let mut jobs = Vec::new();
    for job_id in job_ids {
        if let Ok(job_ctx) = ctx.context_manager.get_context(job_id).await {
            let include = match filter {
                "completed" => job_ctx.state == JobState::Completed,
                "failed" => job_ctx.state == JobState::Failed,
                "active" => job_ctx.state.is_active(),
                _ => true,
            };
            if include {
                jobs.push(json!({
                    "job_id": job_id.to_string(),
                    "title": job_ctx.title,
                    "status": format!("{:?}", job_ctx.state),
                    "created_at": job_ctx.created_at.to_rfc3339()
                }));
            }
        }
    }

    let summary = ctx.context_manager.summary_for(&ctx.user_id).await;

    Ok(json!({
        "jobs": jobs,
        "summary": {
            "total": summary.total,
            "pending": summary.pending,
            "in_progress": summary.in_progress,
            "completed": summary.completed,
            "failed": summary.failed
        }
    }))
}

pub async fn execute_job_status(
    params: &Value,
    ctx: &JobsContext,
) -> Result<Value, JobsCapabilityError> {
    let job_id_str = require_str(params, "job_id")?;
    let job_id = resolve_job_id(job_id_str, &ctx.context_manager).await?;

    match ctx.context_manager.get_context(job_id).await {
        Ok(job_ctx) => {
            if !job_ctx.is_owned_by(&ctx.user_id) {
                return Ok(json!({
                    "error": "Job not found"
                }));
            }
            Ok(json!({
                "job_id": job_id.to_string(),
                "title": job_ctx.title,
                "description": job_ctx.description,
                "status": format!("{:?}", job_ctx.state),
                "created_at": job_ctx.created_at.to_rfc3339(),
                "started_at": job_ctx.started_at.map(|t| t.to_rfc3339()),
                "completed_at": job_ctx.completed_at.map(|t| t.to_rfc3339()),
                "actual_cost": job_ctx.actual_cost.to_string(),
                "fallback_deliverable": job_ctx.metadata.get("fallback_deliverable"),
            }))
        }
        Err(e) => Ok(json!({
            "error": format!("Job not found: {}", e)
        })),
    }
}

pub async fn execute_job_events(
    params: &Value,
    ctx: &JobsContext,
) -> Result<Value, JobsCapabilityError> {
    let job_id_str = require_str(params, "job_id")?;
    let job_id = resolve_job_id(job_id_str, &ctx.context_manager).await?;

    let job_ctx = ctx
        .context_manager
        .get_context(job_id)
        .await
        .map_err(|_| {
            JobsCapabilityError::operation(format!(
                "job {} not found or context unavailable",
                job_id
            ))
        })?;

    if !job_ctx.is_owned_by(&ctx.user_id) {
        return Err(JobsCapabilityError::operation(format!(
            "job {} does not belong to current user",
            job_id
        )));
    }

    let store = ctx.store.as_ref().ok_or_else(|| {
        JobsCapabilityError::operation("no database available to load job events".to_string())
    })?;

    const MAX_EVENT_LIMIT: i64 = 1000;
    let limit = params
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(50)
        .clamp(1, MAX_EVENT_LIMIT);

    let events = store
        .list_job_events(job_id, Some(limit))
        .await
        .map_err(|e| {
            JobsCapabilityError::operation(format!("failed to load job events: {}", e))
        })?;

    let recent: Vec<Value> = events
        .iter()
        .map(|ev| {
            json!({
                "event_type": ev.event_type,
                "data": ev.data,
                "created_at": ev.created_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(json!({
        "job_id": job_id.to_string(),
        "total_events": events.len(),
        "returned": recent.len(),
        "events": recent,
    }))
}

pub async fn execute_job_prompt(
    params: &Value,
    ctx: &JobsContext,
) -> Result<Value, JobsCapabilityError> {
    let job_id_str = require_str(params, "job_id")?;
    let job_id = resolve_job_id(job_id_str, &ctx.context_manager).await?;

    let job_ctx = ctx
        .context_manager
        .get_context(job_id)
        .await
        .map_err(|_| {
            JobsCapabilityError::operation(format!(
                "job {} not found or context unavailable",
                job_id
            ))
        })?;

    if !job_ctx.is_owned_by(&ctx.user_id) {
        return Err(JobsCapabilityError::operation(format!(
            "job {} does not belong to current user",
            job_id
        )));
    }

    let content = require_str(params, "content")?;
    let done = params
        .get("done")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let prompt_queue = ctx.prompt_queue.as_ref().ok_or_else(|| {
        JobsCapabilityError::operation("no prompt queue available".to_string())
    })?;

    let prompt = crate::orchestrator::api::PendingPrompt {
        content: content.to_string(),
        done,
    };

    {
        let mut queue = prompt_queue.lock().await;
        queue.entry(job_id).or_default().push_back(prompt);
    }

    Ok(json!({
        "job_id": job_id.to_string(),
        "status": "queued",
        "message": "Prompt queued",
        "done": done,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_job_descriptor_is_valid() {
        let desc = create_job_descriptor();
        assert_eq!(desc.id.as_str(), CREATE_JOB_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn cancel_job_descriptor_is_valid() {
        let desc = cancel_job_descriptor();
        assert_eq!(desc.id.as_str(), CANCEL_JOB_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn list_jobs_descriptor_is_valid() {
        let desc = list_jobs_descriptor();
        assert_eq!(desc.id.as_str(), LIST_JOBS_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn job_status_descriptor_is_valid() {
        let desc = job_status_descriptor();
        assert_eq!(desc.id.as_str(), JOB_STATUS_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn job_events_descriptor_is_valid() {
        let desc = job_events_descriptor();
        assert_eq!(desc.id.as_str(), JOB_EVENTS_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn job_prompt_descriptor_is_valid() {
        let desc = job_prompt_descriptor();
        assert_eq!(desc.id.as_str(), JOB_PROMPT_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn descriptors_returns_all() {
        let descs = descriptors();
        assert_eq!(descs.len(), 6);
    }
}
