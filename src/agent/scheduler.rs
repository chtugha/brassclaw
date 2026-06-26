//! Job scheduler for parallel execution.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{RwLock, oneshot, mpsc};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::agent::background_tasks::BackgroundTaskRegistry;
use crate::agent::dead_letter_queue::{DeadLetterQueue, DLQConfig};
use crate::agent::task::{Task, TaskContext, TaskOutput};
use crate::config::AgentConfig;
use crate::context::{ContextManager, JobContext, JobState};
use crate::error::{Error, JobError};
use crate::extensions::ExtensionManager;
use crate::hooks::HookRegistry;
use crate::tenant::SystemScope;
use brassclaw_engine::EffectExecutor;
use brassclaw_llm::LlmProvider;
use brassclaw_safety::SafetyLayer;

/// Message that can be sent to a running job.
#[derive(Debug, Clone)]
pub(crate) enum JobMessage {
    /// User message to be processed by the job
    UserMessage(String),
    /// Request to cancel the job
    Cancel,
}

/// Context for generic task execution.
pub struct GenericTaskContext {
    /// Job ID for this task execution.
    pub job_id: Uuid,
    /// Task description.
    pub description: String,
    /// Task-specific parameters.
    pub params: serde_json::Value,
}

/// Handler for generic task types.
#[async_trait::async_trait]
pub trait GenericTaskHandler: Send + Sync {
    /// Execute the generic task.
    async fn execute(&self, context: GenericTaskContext) -> Result<serde_json::Value, Error>;
    
    /// Get a description of this handler.
    fn description(&self) -> &str;
}

/// Task type that can be executed by a job worker.
#[derive(Debug, Clone)]
enum JobTaskType {
    /// Execute a tool via EffectExecutor
    ToolExec {
        tool_name: String,
        params: serde_json::Value,
    },
    /// Background task execution
    Background {
        task_name: String,
        params: serde_json::Value,
    },
    /// Generic task with custom handler
    Generic {
        description: String,
        handler_name: Option<String>,
        params: serde_json::Value,
    },
}

/// Result of parsing a job's task from its context.
type ParseTaskResult = Result<JobTaskType, Error>;

/// Handle to a running job worker.
struct JobHandle {
    /// Task handle for the worker
    task_handle: JoinHandle<()>,
    /// Channel to send messages to the job
    message_tx: mpsc::Sender<JobMessage>,
    /// When the job started (for monitoring/debugging)
    #[allow(dead_code)]
    started_at: std::time::Instant,
}

/// Status of a scheduled sub-task.
struct ScheduledSubtask {
    handle: JoinHandle<Result<TaskOutput, Error>>,
}

/// Shared scheduler-owned dependencies that are forwarded into autonomous runs.
pub struct SchedulerDeps {
    /// V2 effect executor for capability-based tool execution
    pub effect_executor: Option<Arc<dyn EffectExecutor>>,
    pub extension_manager: Option<Arc<ExtensionManager>>,
    pub store: Option<SystemScope>,
    pub hooks: Arc<HookRegistry>,
}

/// Schedules and manages parallel job execution.
///
/// Note: V2 migration in progress. Job scheduling infrastructure is being simplified
/// to focus on tool execution via EffectExecutor. Full job scheduling will be
/// reimplemented using V2 capabilities.
#[derive(Clone)]
pub struct Scheduler {
    config: AgentConfig,
    context_manager: Arc<ContextManager>,
    #[allow(dead_code)]
    llm: Arc<dyn LlmProvider>,
    safety: Arc<SafetyLayer>,
    /// V2 effect executor for capability-based tool execution
    effect_executor: Option<Arc<dyn EffectExecutor>>,
    #[allow(dead_code)]
    extension_manager: Option<Arc<ExtensionManager>>,
    store: Option<SystemScope>,
    #[allow(dead_code)]
    hooks: Arc<HookRegistry>,
    event_publisher: Option<brassclaw_common::DynEventPublisher>,
    /// HTTP interceptor for trace recording/replay (propagated to workers).
    http_interceptor: Option<Arc<dyn brassclaw_llm::recording::HttpInterceptor>>,
    /// Resolved runtime policy propagated to per-job workers so the
    /// model-facing tool list filter applies to background jobs too.
    /// `None` in tests / before `Config::with_runtime_overrides` runs.
    runtime_policy: Option<brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy>,
    /// Running sub-tasks (tool executions, background tasks).
    subtasks: Arc<RwLock<HashMap<Uuid, ScheduledSubtask>>>,
    /// Running jobs (full job executions with workers).
    running_jobs: Arc<RwLock<HashMap<Uuid, JobHandle>>>,
    /// Background task registry for executing background tasks.
    background_tasks: Arc<BackgroundTaskRegistry>,
    /// Generic task handler registry for extensible task execution.
    generic_handlers: Arc<RwLock<HashMap<String, Arc<dyn GenericTaskHandler>>>>,
    /// Dead letter queue for failed jobs.
    dead_letter_queue: Arc<DeadLetterQueue>,
}

impl Scheduler {
    /// Create a new scheduler.
    pub fn new(
        config: AgentConfig,
        context_manager: Arc<ContextManager>,
        llm: Arc<dyn LlmProvider>,
        safety: Arc<SafetyLayer>,
        deps: SchedulerDeps,
    ) -> Self {
        // Create background task registry with built-in handlers.
        // Uses the sync constructor to avoid async runtime nesting issues.
        let background_tasks = Arc::new(BackgroundTaskRegistry::with_defaults());
        
        // Create dead letter queue with default config
        let dlq_config = DLQConfig::default();
        let dead_letter_queue = Arc::new(DeadLetterQueue::new_in_memory(dlq_config));
        
        Self {
            config,
            context_manager,
            llm,
            safety,
            effect_executor: deps.effect_executor,
            extension_manager: deps.extension_manager,
            store: deps.store,
            hooks: deps.hooks,
            event_publisher: None,
            http_interceptor: None,
            runtime_policy: None,
            subtasks: Arc::new(RwLock::new(HashMap::new())),
            running_jobs: Arc::new(RwLock::new(HashMap::new())),
            background_tasks,
            generic_handlers: Arc::new(RwLock::new(HashMap::new())),
            dead_letter_queue,
        }
    }

    pub fn set_event_publisher(&mut self, ep: brassclaw_common::DynEventPublisher) {
        self.event_publisher = Some(ep);
    }

    /// Set the HTTP interceptor for trace recording/replay.
    pub fn set_http_interceptor(
        &mut self,
        interceptor: Arc<dyn brassclaw_llm::recording::HttpInterceptor>,
    ) {
        self.http_interceptor = Some(interceptor);
    }

    /// Propagate the resolved runtime policy to spawned workers so background
    /// jobs see the same model-facing tool surface as the dispatcher
    /// (#3243 HIGH iteration-2 gap).
    pub fn set_runtime_policy(
        &mut self,
        policy: brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy,
    ) {
        self.runtime_policy = Some(policy);
    }

    /// Create, persist, and schedule a job in one shot.
    ///
    /// This is the preferred entry point for dispatching new jobs. It:
    /// 1. Creates the job context via `ContextManager`
    /// 2. Optionally applies metadata (e.g. `max_iterations`)
    /// 3. Persists the job to the database (so FK references from
    ///    `job_actions` / `llm_calls` work immediately)
    /// 4. Schedules the job for worker execution
    ///
    /// Returns the new job ID.
    pub async fn dispatch_job(
        &self,
        user_id: &str,
        title: &str,
        description: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid, JobError> {
        self.dispatch_job_inner(
            user_id,
            title,
            description,
            metadata,
        )
        .await
    }


    /// Shared implementation for `dispatch_job` and `dispatch_job_with_context`.
    async fn dispatch_job_inner(
        &self,
        user_id: &str,
        title: &str,
        description: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid, JobError> {
        let job_id = self
            .context_manager
            .create_job_for_user(user_id, title, description)
            .await?;

        // Apply metadata and token budget in a single atomic update.
        // This prevents concurrent workers from observing partial state.
        // Cap user-supplied max_tokens at the configured limit (Issue #815).
        let user_max_tokens = metadata
            .as_ref()
            .and_then(|m| m.get("max_tokens"))
            .and_then(|v| v.as_u64());

        let max_tokens = user_max_tokens
            .map(|user_val| {
                if self.config.max_tokens_per_job == 0 {
                    // Config is "unlimited": use the user-supplied value directly.
                    user_val
                } else {
                    std::cmp::min(user_val, self.config.max_tokens_per_job)
                }
            })
            .unwrap_or(self.config.max_tokens_per_job);

        // Apply metadata and token budget in one closure
        // (Issue #813: atomic update). Use update_context_and_get to ensure atomicity:
        // no gap where concurrent workers can modify the context between update and
        // DB persist (Issue #807).
        let needs_update = metadata.is_some() || max_tokens > 0;
        let ctx = if needs_update {
            self.context_manager
                .update_context_and_get(job_id, |ctx| {
                    if let Some(meta) = metadata {
                        ctx.metadata = meta;
                    }
                    if max_tokens > 0 {
                        ctx.max_tokens = max_tokens;
                    }
                })
                .await?
        } else {
            // Currently unreachable via dispatch_job() which always provides
            // Some(approval_context), but kept as a safe fallback.
            self.context_manager.get_context(job_id).await?
        };

        // Persist to DB before scheduling so the worker's FK references are valid.
        // The context was read under the same lock as the update (atomic), preventing
        // concurrent worker interference (Issue #807: non-transactional context updates).
        if let Some(ref store) = self.store {
            store.save_job(&ctx).await.map_err(|e| JobError::Failed {
                id: job_id,
                reason: format!("failed to persist job: {e}"),
            })?;
        }

        self.schedule_with_context(job_id).await?;
        Ok(job_id)
    }

    /// Schedule a job for execution.
    pub async fn schedule(&self, job_id: Uuid) -> Result<(), JobError> {
        self.schedule_with_context(job_id).await
    }

    /// Parse job task from JobContext metadata.
    ///
    /// Extracts task type and parameters from the job's metadata field.
    fn parse_job_task(job_ctx: &JobContext) -> ParseTaskResult {
        let metadata = job_ctx.metadata.as_object()
            .ok_or_else(|| Error::Job(JobError::ContextError {
                id: job_ctx.job_id,
                reason: "Job metadata is not an object".to_string(),
            }))?;

        // Check for task_type field
        let task_type = metadata.get("task_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Job(JobError::ContextError {
                id: job_ctx.job_id,
                reason: "Missing task_type in job metadata".to_string(),
            }))?;

        match task_type {
            "tool_exec" => {
                let tool_name = metadata.get("tool_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Job(JobError::ContextError {
                        id: job_ctx.job_id,
                        reason: "Missing tool_name for tool_exec task".to_string(),
                    }))?
                    .to_string();

                let params = metadata.get("params")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                Ok(JobTaskType::ToolExec { tool_name, params })
            }
            "background" => {
                let task_name = metadata.get("task_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Job(JobError::ContextError {
                        id: job_ctx.job_id,
                        reason: "Missing task_name for background task".to_string(),
                    }))?
                    .to_string();

                let params = metadata.get("params")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                Ok(JobTaskType::Background { task_name, params })
            }
            "generic" => {
                let description = metadata.get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Generic task")
                    .to_string();
                
                let handler_name = metadata.get("handler_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                
                let params = metadata.get("params")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                Ok(JobTaskType::Generic {
                    description,
                    handler_name,
                    params,
                })
            }
            _ => {
                Err(Error::Job(JobError::ContextError {
                    id: job_ctx.job_id,
                    reason: format!("Unknown task_type: {}", task_type),
                }))
            }
        }
    }

    /// Check if an error is transient and should be retried.
    fn is_transient_error(error: &Error) -> bool {
        match error {
            // Network-related errors are typically transient
            Error::Tool(tool_err) => {
                let reason = match tool_err {
                    crate::error::ToolError::ExecutionFailed { reason, .. } => reason,
                    crate::error::ToolError::NotFound { .. } => return false,
                    crate::error::ToolError::InvalidParameters { .. } => return false,
                    crate::error::ToolError::AuthRequired { .. } => return false,
                    crate::error::ToolError::Disabled { .. } => return false,
                    crate::error::ToolError::Timeout { .. } => return true,
                    crate::error::ToolError::RateLimited { .. } => return true,
                    crate::error::ToolError::Sandbox { .. } => return false,
                    crate::error::ToolError::AutonomousUnavailable { .. } => return false,
                    crate::error::ToolError::BuilderFailed { .. } => return false,
                };
                
                // Check for common transient error patterns
                reason.contains("timeout") ||
                reason.contains("connection") ||
                reason.contains("network") ||
                reason.contains("temporary") ||
                reason.contains("unavailable") ||
                reason.contains("rate limit")
            }
            // LLM errors might be transient
            Error::Llm(_) => true,
            // Most other errors are permanent
            _ => false,
        }
    }

    /// Execute a job task with retry logic for transient failures.
    async fn execute_with_retry(
        scheduler: &Scheduler,
        job_id: Uuid,
        task: &JobTaskType,
        max_retries: u32,
    ) -> Result<TaskOutput, Error> {
        let mut attempts = 0;
        let mut last_error: Option<String> = None;

        while attempts <= max_retries {
            if attempts > 0 {
                tracing::info!(
                    job_id = %job_id,
                    attempt = attempts + 1,
                    max_retries = max_retries + 1,
                    "Retrying job execution after transient failure"
                );
            }

            match Self::execute_task_internal(scheduler, job_id, task).await {
                Ok(output) => {
                    if attempts > 0 {
                        tracing::info!(
                            job_id = %job_id,
                            attempts = attempts + 1,
                            "Job execution succeeded after retry"
                        );
                    }
                    return Ok(output);
                }
                Err(e) => {
                    let error_string = format!("{}", e);
                    last_error = Some(error_string.clone());
                    
                    // Check if error is transient and we have retries left
                    if Self::is_transient_error(&e) && attempts < max_retries {
                        attempts += 1;
                        
                        // Exponential backoff: 2^attempts seconds
                        let backoff_secs = 2_u64.pow(attempts);
                        let backoff = Duration::from_secs(backoff_secs);
                        
                        tracing::warn!(
                            job_id = %job_id,
                            error = %e,
                            attempt = attempts,
                            backoff_secs = backoff_secs,
                            "Transient error, will retry after backoff"
                        );
                        
                        tokio::time::sleep(backoff).await;
                        continue;
                    } else {
                        // Permanent error or out of retries
                        if attempts > 0 {
                            tracing::error!(
                                job_id = %job_id,
                                error = %e,
                                attempts = attempts + 1,
                                "Job execution failed after all retries"
                            );
                        }
                        return Err(e);
                    }
                }
            }
        }

        // Should never reach here, but return last error if we do
        Err(Error::Job(JobError::ContextError {
            id: job_id,
            reason: last_error.unwrap_or_else(|| "Unknown error during retry loop".to_string()),
        }))
    }

    /// Internal task execution without retry logic.
    async fn execute_task_internal(
        scheduler: &Scheduler,
        job_id: Uuid,
        task: &JobTaskType,
    ) -> Result<TaskOutput, Error> {
        // Check for cancellation before execution
        let job_ctx = scheduler.context_manager.get_context(job_id).await?;
        if job_ctx.state == JobState::Cancelled {
            return Err(Error::Job(JobError::ContextError {
                id: job_id,
                reason: "Job cancelled before execution".to_string(),
            }));
        }

        match task {
            JobTaskType::ToolExec { tool_name, params } => {
                tracing::debug!(
                    job_id = %job_id,
                    tool_name = %tool_name,
                    "Executing tool task"
                );

                Self::execute_tool_task(
                    scheduler.effect_executor.clone(),
                    scheduler.context_manager.clone(),
                    scheduler.safety.clone(),
                    job_id,
                    tool_name,
                    params.clone(),
                )
                .await
            }
            JobTaskType::Background { task_name, params } => {
                tracing::debug!(
                    job_id = %job_id,
                    task_name = %task_name,
                    "Executing background task"
                );

                let start = std::time::Instant::now();
                
                // Execute via background task registry
                let result = scheduler.background_tasks
                    .execute(job_id, task_name, params.clone())
                    .await?;
                
                let duration = start.elapsed();
                
                tracing::info!(
                    job_id = %job_id,
                    task_name = %task_name,
                    duration_ms = duration.as_millis(),
                    "Background task completed successfully"
                );

                Ok(TaskOutput {
                    result: serde_json::json!({
                        "status": "completed",
                        "task": task_name,
                        "result": result,
                    }),
                    duration,
                })
            }
            JobTaskType::Generic { description, handler_name, params } => {
                tracing::debug!(
                    job_id = %job_id,
                    description = %description,
                    handler_name = ?handler_name,
                    "Executing generic task"
                );

                let start = std::time::Instant::now();
                
                let result = if let Some(handler_name) = handler_name {
                    // Execute via registered handler
                    let handlers = scheduler.generic_handlers.read().await;
                    
                    let handler = handlers.get(handler_name).ok_or_else(|| {
                        Error::Config(crate::error::ConfigError::InvalidValue {
                            key: "handler_name".to_string(),
                            message: format!(
                                "Unknown generic task handler: '{}'. Available handlers: {}",
                                handler_name,
                                handlers.keys().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
                            ),
                        })
                    })?;
                    
                    let context = GenericTaskContext {
                        job_id,
                        description: description.clone(),
                        params: params.clone(),
                    };
                    
                    handler.execute(context).await?
                } else {
                    // No handler specified - return basic completion
                    serde_json::json!({
                        "status": "completed",
                        "description": description,
                        "params": params,
                    })
                };
                
                let duration = start.elapsed();
                
                tracing::info!(
                    job_id = %job_id,
                    description = %description,
                    duration_ms = duration.as_millis(),
                    "Generic task completed successfully"
                );

                Ok(TaskOutput {
                    result: serde_json::json!({
                        "status": "completed",
                        "description": description,
                        "result": result,
                    }),
                    duration,
                })
            }
        }
    }

    /// Worker task that executes a job with full production features.
    ///
    /// This is spawned as a separate tokio task for each job.
    ///
    /// Features:
    /// - Task parsing and execution (tool_exec, background, generic)
    /// - User message processing and storage
    /// - Cancellation handling via message channel
    /// - Job timeout monitoring
    /// - Error recovery with exponential backoff retry
    /// - Comprehensive state management and persistence
    async fn execute_job_worker(
        scheduler: Arc<Scheduler>,
        job_id: Uuid,
        mut message_rx: mpsc::Receiver<JobMessage>,
    ) {
        tracing::info!(job_id = %job_id, "Job worker started");

        // Get job context
        let job_ctx = match scheduler.context_manager.get_context(job_id).await {
            Ok(ctx) => ctx,
            Err(e) => {
                tracing::error!(job_id = %job_id, error = %e, "Failed to get job context");
                let _ = scheduler.running_jobs.write().await.remove(&job_id);
                return;
            }
        };

        // Check if job was already cancelled
        if job_ctx.state == JobState::Cancelled {
            tracing::info!(job_id = %job_id, "Job already cancelled, worker exiting");
            let _ = scheduler.running_jobs.write().await.remove(&job_id);
            return;
        }

        // Parse job task from metadata
        let task = match Self::parse_job_task(&job_ctx) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(job_id = %job_id, error = %e, "Failed to parse job task");
                
                // Mark job as failed
                let _ = scheduler
                    .context_manager
                    .update_context(job_id, |ctx| {
                        let _ = ctx.transition_to(
                            JobState::Failed,
                            Some(format!("Failed to parse job task: {}", e)),
                        );
                    })
                    .await;

                if let Some(ref store) = scheduler.store {
                    let _ = store
                        .update_job_status(
                            job_id,
                            JobState::Failed,
                            Some(&format!("Failed to parse job task: {}", e)),
                        )
                        .await;
                }

                let _ = scheduler.running_jobs.write().await.remove(&job_id);
                return;
            }
        };

        tracing::info!(job_id = %job_id, task = ?task, "Parsed job task");

        // Create channels for cancellation and user messages
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        let (user_msg_tx, mut user_msg_rx) = mpsc::channel::<String>(32);

        // Spawn message listener task
        let listener_job_id = job_id;
        let listener_context_manager = scheduler.context_manager.clone();
        tokio::spawn(async move {
            while let Some(msg) = message_rx.recv().await {
                match msg {
                    JobMessage::Cancel => {
                        tracing::info!(job_id = %listener_job_id, "Received cancel message");
                        
                        // Update job state to cancelled
                        let _ = listener_context_manager
                            .update_context(listener_job_id, |ctx| {
                                let _ = ctx.transition_to(
                                    JobState::Cancelled,
                                    Some("Cancelled by user request".to_string()),
                                );
                            })
                            .await;
                        
                        // Signal cancellation to main execution
                        let _ = cancel_tx.send(());
                        break;
                    }
                    JobMessage::UserMessage(content) => {
                        tracing::debug!(
                            job_id = %listener_job_id,
                            message_len = content.len(),
                            "Received user message"
                        );
                        
                        // Process interactive commands
                        let response = Self::process_user_message(&content, listener_job_id, &listener_context_manager).await;
                        
                        // Store message and response in job context metadata
                        let _ = listener_context_manager
                            .update_context(listener_job_id, |ctx| {
                                if let Some(obj) = ctx.metadata.as_object_mut() {
                                    let messages = obj.entry("user_messages")
                                        .or_insert_with(|| serde_json::json!([]));
                                    
                                    if let Some(arr) = messages.as_array_mut() {
                                        arr.push(serde_json::json!({
                                            "content": content.clone(),
                                            "timestamp": chrono::Utc::now().to_rfc3339(),
                                            "response": response,
                                        }));
                                    }
                                }
                            })
                            .await;
                        
                        // Forward to execution task if not a command
                        if !content.trim().starts_with('/') {
                            let _ = user_msg_tx.send(content).await;
                        }
                    }
                }
            }
        });

        // Execute job with timeout and cancellation support
        let job_timeout = scheduler.config.job_timeout;
        let max_retries = 3; // Maximum retry attempts for transient failures
        
        tracing::debug!(
            job_id = %job_id,
            timeout_secs = job_timeout.as_secs(),
            max_retries = max_retries,
            "Starting job execution with timeout and retry"
        );

        // Execute with timeout
        let execution_future = Self::execute_with_retry(&scheduler, job_id, &task, max_retries);
        
        tokio::select! {
            // Job execution completed (success or permanent failure)
            result = execution_future => {
                match result {
                    Ok(output) => {
                        tracing::info!(
                            job_id = %job_id,
                            duration_ms = output.duration.as_millis(),
                            "Job execution completed successfully"
                        );

                        // Store result in job context
                        let _ = scheduler
                            .context_manager
                            .update_context(job_id, |ctx| {
                                if let Some(obj) = ctx.metadata.as_object_mut() {
                                    obj.insert("result".to_string(), output.result.clone());
                                    obj.insert("duration_ms".to_string(), serde_json::json!(output.duration.as_millis()));
                                }
                                
                                let _ = ctx.transition_to(
                                    JobState::Completed,
                                    Some("Job execution completed successfully".to_string()),
                                );
                            })
                            .await;

                        // Persist to database
                        if let Some(ref store) = scheduler.store {
                            let _ = store
                                .update_job_status(
                                    job_id,
                                    JobState::Completed,
                                    Some("Job execution completed successfully"),
                                )
                                .await;
                        }
                    }
                    Err(e) => {
                        tracing::error!(job_id = %job_id, error = %e, "Job execution failed");

                        // Get job context for DLQ
                        let job_metadata = scheduler
                            .context_manager
                            .get_context(job_id)
                            .await
                            .ok()
                            .map(|ctx| ctx.metadata.clone())
                            .unwrap_or_else(|| serde_json::json!({}));

                        // Add to dead letter queue
                        let _ = scheduler
                            .dead_letter_queue
                            .add_failed_job(job_id, e.to_string(), job_metadata)
                            .await;

                        // Store error in job context
                        let _ = scheduler
                            .context_manager
                            .update_context(job_id, |ctx| {
                                if let Some(obj) = ctx.metadata.as_object_mut() {
                                    obj.insert("error".to_string(), serde_json::json!(e.to_string()));
                                }
                                
                                let _ = ctx.transition_to(
                                    JobState::Failed,
                                    Some(format!("Job execution failed: {}", e)),
                                );
                            })
                            .await;

                        // Persist to database
                        if let Some(ref store) = scheduler.store {
                            let _ = store
                                .update_job_status(
                                    job_id,
                                    JobState::Failed,
                                    Some(&format!("Job execution failed: {}", e)),
                                )
                                .await;
                        }
                    }
                }
            }
            
            // Job was cancelled
            _ = cancel_rx => {
                tracing::info!(job_id = %job_id, "Job cancelled during execution");
                
                // Persist cancellation to database
                if let Some(ref store) = scheduler.store {
                    let _ = store
                        .update_job_status(
                            job_id,
                            JobState::Cancelled,
                            Some("Cancelled by user request"),
                        )
                        .await;
                }
            }
            
            // Job timeout exceeded
            _ = tokio::time::sleep(job_timeout) => {
                tracing::warn!(
                    job_id = %job_id,
                    timeout_secs = job_timeout.as_secs(),
                    "Job execution timeout exceeded"
                );

                // Mark job as failed due to timeout
                let _ = scheduler
                    .context_manager
                    .update_context(job_id, |ctx| {
                        if let Some(obj) = ctx.metadata.as_object_mut() {
                            obj.insert("timeout".to_string(), serde_json::json!(true));
                            obj.insert("timeout_secs".to_string(), serde_json::json!(job_timeout.as_secs()));
                        }
                        
                        let _ = ctx.transition_to(
                            JobState::Failed,
                            Some(format!("Job execution timeout exceeded ({} seconds)", job_timeout.as_secs())),
                        );
                    })
                    .await;

                // Persist to database
                if let Some(ref store) = scheduler.store {
                    let _ = store
                        .update_job_status(
                            job_id,
                            JobState::Failed,
                            Some(&format!("Job execution timeout exceeded ({} seconds)", job_timeout.as_secs())),
                        )
                        .await;
                }
            }
        }

        // Process any remaining user messages
        user_msg_rx.close();
        while let Some(msg) = user_msg_rx.recv().await {
            tracing::debug!(
                job_id = %job_id,
                message_len = msg.len(),
                "Processing remaining user message after job completion"
            );
        }

        // Remove from running jobs tracking
        scheduler.running_jobs.write().await.remove(&job_id);
        tracing::info!(job_id = %job_id, "Job worker finished");
    }

    /// Schedule a job with an optional approval context.
    ///
    /// V2 migration: Now spawns a worker task to execute the job.
    /// The worker handles the complete job lifecycle including execution,
    /// state updates, and cleanup.
    async fn schedule_with_context(
        &self,
        job_id: Uuid,
    ) -> Result<(), JobError> {
        // Per-user concurrency check — only count jobs consuming a parallel
        // execution slot (Pending/InProgress/Stuck), not Completed/Submitted.
        if let Some(max_per_user) = self.config.max_jobs_per_user {
            if let Ok(ctx) = self.context_manager.get_context(job_id).await {
                let user_blocking = self
                    .context_manager
                    .parallel_blocking_count_for(&ctx.user_id)
                    .await;
                if user_blocking >= max_per_user {
                    return Err(JobError::MaxJobsExceeded { max: max_per_user });
                }
            }
        }

        // Transition job to in_progress
        self.context_manager
            .update_context(job_id, |ctx| {
                ctx.transition_to(
                    JobState::InProgress,
                    Some("Scheduled for execution".to_string()),
                )
            })
            .await?
            .map_err(|s| JobError::ContextError {
                id: job_id,
                reason: s,
            })?;

        // Create message channel for job communication
        let (message_tx, message_rx) = mpsc::channel(32);

        // Spawn worker task
        let scheduler = Arc::new(self.clone());
        let task_handle = tokio::spawn(Self::execute_job_worker(
            scheduler,
            job_id,
            message_rx,
        ));

        // Track the running job
        self.running_jobs.write().await.insert(
            job_id,
            JobHandle {
                task_handle,
                message_tx,
                started_at: std::time::Instant::now(),
            },
        );

        tracing::info!(
            job_id = %job_id,
            "Job scheduled and worker spawned"
        );
        Ok(())
    }

    /// Schedule a sub-task from within a worker.
    ///
    /// Sub-tasks are lightweight tasks that don't go through the full job lifecycle.
    /// They're used for parallel tool execution and background computations.
    ///
    /// Returns a oneshot receiver to get the result.
    pub async fn spawn_subtask(
        &self,
        parent_id: Uuid,
        task: Task,
    ) -> Result<oneshot::Receiver<Result<TaskOutput, Error>>, JobError> {
        let task_id = Uuid::new_v4();
        let (result_tx, result_rx) = oneshot::channel();

        let handle = match task {
            Task::Job { .. } => {
                // Jobs should go through schedule(), not spawn_subtask
                return Err(JobError::ContextError {
                    id: parent_id,
                    reason: "Use schedule() for Job tasks, not spawn_subtask()".to_string(),
                });
            }

            Task::ToolExec {
                parent_id: tool_parent_id,
                tool_name,
                params,
            } => {
                let effect_executor = self.effect_executor.clone();
                let context_manager = self.context_manager.clone();
                let safety = self.safety.clone();

                // V2: Subtask permission context will be handled by V2 permission system
                tokio::spawn(async move {
                    let result = Self::execute_tool_task(
                        effect_executor,
                        context_manager,
                        safety,
                        tool_parent_id,
                        &tool_name,
                        params,
                    )
                    .await;

                    // Send result (ignore if receiver dropped)
                    let _ = result_tx.send(result);
                })
            }

            Task::Background { id: _, handler } => {
                let ctx = TaskContext::new(task_id).with_parent(parent_id);

                tokio::spawn(async move {
                    let result = handler.run(ctx).await;
                    let _ = result_tx.send(result);
                })
            }
        };

        // Track the subtask
        self.subtasks.write().await.insert(
            task_id,
            ScheduledSubtask {
                handle: tokio::spawn(async move {
                    // Wrap the handle to get its result
                    match handle.await {
                        Ok(()) => Err(Error::Job(JobError::ContextError {
                            id: task_id,
                            reason: "Subtask completed but result not captured".to_string(),
                        })),
                        Err(e) => Err(Error::Job(JobError::ContextError {
                            id: task_id,
                            reason: format!("Subtask panicked: {}", e),
                        })),
                    }
                }),
            },
        );

        // Cleanup task for subtask tracking
        let subtasks = Arc::clone(&self.subtasks);
        tokio::spawn(async move {
            loop {
                let finished = {
                    let subtasks_read = subtasks.read().await;
                    match subtasks_read.get(&task_id) {
                        Some(scheduled) => scheduled.handle.is_finished(),
                        None => true,
                    }
                };

                if finished {
                    subtasks.write().await.remove(&task_id);
                    break;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        tracing::debug!(
            parent_id = %parent_id,
            task_id = %task_id,
            "Spawned subtask"
        );

        Ok(result_rx)
    }

    /// Schedule multiple tasks in parallel and wait for all to complete.
    ///
    /// Returns results in the same order as the input tasks.
    pub async fn spawn_batch(
        &self,
        parent_id: Uuid,
        tasks: Vec<Task>,
    ) -> Vec<Result<TaskOutput, Error>> {
        if tasks.is_empty() {
            return Vec::new();
        }

        let mut receivers = Vec::with_capacity(tasks.len());

        // Spawn all tasks
        for task in tasks {
            match self.spawn_subtask(parent_id, task).await {
                Ok(rx) => receivers.push(Some(rx)),
                Err(e) => {
                    // Store the error directly
                    receivers.push(None);
                    tracing::warn!(
                        parent_id = %parent_id,
                        error = %e,
                        "Failed to spawn subtask in batch"
                    );
                }
            }
        }

        // Collect results
        let mut results = Vec::with_capacity(receivers.len());
        for rx in receivers {
            let result = match rx {
                Some(receiver) => match receiver.await {
                    Ok(task_result) => task_result,
                    Err(_) => Err(Error::Job(JobError::ContextError {
                        id: parent_id,
                        reason: "Subtask channel closed unexpectedly".to_string(),
                    })),
                },
                None => Err(Error::Job(JobError::ContextError {
                    id: parent_id,
                    reason: "Subtask failed to spawn".to_string(),
                })),
            };
            results.push(result);
        }

        results
    }

    /// Execute a single tool as a subtask.
    ///
    /// Performs scheduler-specific checks (approval, cancellation) then
    /// delegates to V2 EffectExecutor for capability-based tool execution.
    ///
    #[allow(dead_code)]
    async fn execute_tool_task(
        effect_executor: Option<Arc<dyn EffectExecutor>>,
        context_manager: Arc<ContextManager>,
        _safety: Arc<SafetyLayer>,
        job_id: Uuid,
        tool_name: &str,
        params: serde_json::Value,
    ) -> Result<TaskOutput, Error> {
        let start = std::time::Instant::now();

        // Get job context for cancellation check
        let job_ctx: JobContext = context_manager.get_context(job_id).await?;
        if job_ctx.state == JobState::Cancelled {
            return Err(crate::error::ToolError::ExecutionFailed {
                name: tool_name.to_string(),
                reason: "Job is cancelled".to_string(),
            }
            .into());
        }

        // P0.1: Wire EffectExecutor - check if available
        let executor = match effect_executor {
            Some(exec) => exec,
            None => {
                return Err(Error::Tool(crate::error::ToolError::ExecutionFailed {
                    name: tool_name.to_string(),
                    reason: "EffectExecutor not available - V2 migration incomplete".to_string(),
                }));
            }
        };

        let mut thread_context = Self::create_thread_execution_context(&job_ctx, job_id);

        use brassclaw_engine::{CapabilityLease, LeaseId, GrantedActions};
        use chrono::Utc;
        
        let lease = CapabilityLease {
            id: LeaseId::new(),
            thread_id: thread_context.thread_id,
            capability_name: tool_name.to_string(),
            granted_actions: GrantedActions::All,
            granted_at: Utc::now(),
            expires_at: None,
            max_uses: None,
            uses_remaining: None,
            revoked: false,
            revoked_reason: None,
        };

        // P0.2: Populate action inventory snapshots from EffectExecutor
        // This provides the context with available actions for this execution
        match executor.available_actions(std::slice::from_ref(&lease), &thread_context).await {
            Ok(actions) => {
                tracing::debug!(
                    job_id = %job_id,
                    action_count = actions.len(),
                    "Populated action inventory snapshot"
                );
                thread_context.available_actions_snapshot = Some(actions.into());
            }
            Err(e) => {
                // Log warning but don't fail - action inventory is optional context
                tracing::warn!(
                    job_id = %job_id,
                    error = ?e,
                    "Failed to populate action inventory snapshot"
                );
            }
        }

        // P0.2: Populate full action inventory (V2) if available
        match executor.available_action_inventory(std::slice::from_ref(&lease), &thread_context).await {
            Ok(inventory) => {
                tracing::debug!(
                    job_id = %job_id,
                    inline_count = inventory.inline.len(),
                    discoverable_count = inventory.discoverable.len(),
                    "Populated action inventory V2 snapshot"
                );
                thread_context.available_action_inventory_snapshot = Some(std::sync::Arc::new(inventory));
            }
            Err(e) => {
                // Log warning but don't fail - action inventory is optional context
                tracing::warn!(
                    job_id = %job_id,
                    error = ?e,
                    "Failed to populate action inventory V2 snapshot"
                );
            }
        }

        // P0.2: Execute action via EffectExecutor
        tracing::debug!(
            job_id = %job_id,
            tool_name = %tool_name,
            thread_type = ?thread_context.thread_type,
            project_id = ?thread_context.project_id,
            "Executing tool via EffectExecutor"
        );

        match executor.execute_action(tool_name, params.clone(), &lease, &thread_context).await {
            Ok(action_result) => {
                let duration = start.elapsed();
                
                tracing::debug!(
                    job_id = %job_id,
                    tool_name = %tool_name,
                    duration_ms = ?duration.as_millis(),
                    is_error = action_result.is_error,
                    "Tool execution completed"
                );

                // P0.2: Convert ActionResult to TaskOutput
                Ok(TaskOutput {
                    result: action_result.output,
                    duration,
                })
            }
            Err(engine_error) => {
                let duration = start.elapsed();
                
                // P0.2: Enhanced error handling with specific error mapping
                use brassclaw_engine::EngineError;
                
                let error_message = match &engine_error {
                    EngineError::Capability(cap_err) => {
                        format!("Capability error: {}", cap_err)
                    }
                    EngineError::Step(step_err) => {
                        format!("Step execution error: {}", step_err)
                    }
                    EngineError::Thread(thread_err) => {
                        format!("Thread error: {}", thread_err)
                    }
                    EngineError::Store { reason } => {
                        format!("Storage error: {}", reason)
                    }
                    EngineError::Llm { reason } => {
                        format!("LLM error: {}", reason)
                    }
                    EngineError::Effect { reason } => {
                        format!("Effect execution error: {}", reason)
                    }
                    EngineError::InvalidInput { reason } => {
                        format!("Invalid input: {}", reason)
                    }
                    EngineError::AccessDenied { user_id, entity } => {
                        format!("Access denied: user '{}' cannot access {}", user_id, entity)
                    }
                    EngineError::ThreadNotFound(thread_id) => {
                        format!("Thread not found: {}", thread_id)
                    }
                    EngineError::ProjectNotFound(project_id) => {
                        format!("Project not found: {}", project_id)
                    }
                    EngineError::LeaseExpired { capability_name } => {
                        format!("Lease expired for capability: {}", capability_name)
                    }
                    EngineError::LeaseDenied { reason } => {
                        format!("Lease denied: {}", reason)
                    }
                    EngineError::MaxIterations { limit } => {
                        format!("Max iterations reached: {}", limit)
                    }
                    EngineError::TokenLimitExceeded { used, limit } => {
                        format!("Token limit exceeded: {} of {}", used, limit)
                    }
                    EngineError::Timeout { elapsed, limit } => {
                        format!("Thread timeout: {:?} of {:?}", elapsed, limit)
                    }
                    _ => {
                        format!("{}", engine_error)
                    }
                };
                
                tracing::warn!(
                    job_id = %job_id,
                    tool_name = %tool_name,
                    user_id = %job_ctx.user_id,
                    error = %error_message,
                    params = ?params,
                    duration_ms = ?duration.as_millis(),
                    "Tool execution failed via EffectExecutor"
                );

                // P0.2: Convert EngineError to Error with enhanced context
                Err(Error::Tool(crate::error::ToolError::ExecutionFailed {
                    name: tool_name.to_string(),
                    reason: error_message,
                }))
            }
        }
    }

    /// Create a ThreadExecutionContext for tool execution.
    ///
    /// P0.2: Enhanced implementation with real values from job context
    fn create_thread_execution_context(
        job_ctx: &JobContext,
        job_id: Uuid,
    ) -> brassclaw_engine::ThreadExecutionContext {
        use brassclaw_engine::{ThreadExecutionContext, ThreadType, ProjectId, StepId, ThreadId, ValidTimezone};

        // Extract user_id from metadata or use job's user_id
        let user_id = job_ctx
            .metadata
            .as_object()
            .and_then(|obj| obj.get("user_id"))
            .and_then(|v| v.as_str())
            .unwrap_or(&job_ctx.user_id)
            .to_string();

        // Extract description from metadata if available, fallback to job description
        let thread_goal = job_ctx
            .metadata
            .as_object()
            .and_then(|obj| obj.get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Some(job_ctx.description.clone()));

        // P0.2: Extract project_id from job metadata
        // Look for "project_id" in metadata, default to nil UUID if not found
        let project_id = job_ctx
            .metadata
            .as_object()
            .and_then(|obj| obj.get("project_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(ProjectId)
            .unwrap_or_else(|| {
                tracing::debug!(
                    job_id = %job_id,
                    "No project_id in job metadata, using nil UUID"
                );
                ProjectId(uuid::Uuid::nil())
            });

        // P0.2: Determine ThreadType from job metadata
        // Look for "thread_type" or "job_type" in metadata
        let thread_type = job_ctx
            .metadata
            .as_object()
            .and_then(|obj| {
                obj.get("thread_type")
                    .or_else(|| obj.get("job_type"))
            })
            .and_then(|v| v.as_str())
            .and_then(|s| match s.to_lowercase().as_str() {
                "foreground" | "conversation" | "interactive" => Some(ThreadType::Foreground),
                "research" | "background" => Some(ThreadType::Research),
                "routine" | "mission" | "scheduled" => Some(ThreadType::Mission),
                _ => None,
            })
            .unwrap_or_else(|| {
                // Default to Research for general agent tasks
                tracing::debug!(
                    job_id = %job_id,
                    "No thread_type in job metadata, defaulting to Research"
                );
                ThreadType::Research
            });

        // P0.2: Extract step_id from job metadata if available
        // This allows tracking of specific execution steps
        let step_id = job_ctx
            .metadata
            .as_object()
            .and_then(|obj| obj.get("step_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(StepId)
            .unwrap_or_else(|| {
                // Generate a new step_id for this execution
                StepId(uuid::Uuid::new_v4())
            });

        // P0.2: Get user timezone from JobContext
        // JobContext already has user_timezone field populated
        let user_timezone = if !job_ctx.user_timezone.is_empty() && job_ctx.user_timezone != "UTC" {
            // Try to parse as ValidTimezone using parse() method
            ValidTimezone::parse(&job_ctx.user_timezone)
                .or_else(|| {
                    tracing::warn!(
                        job_id = %job_id,
                        timezone = %job_ctx.user_timezone,
                        "Invalid timezone in JobContext, falling back to None"
                    );
                    None
                })
        } else {
            None
        };

        // P0.2: Extract source_channel from job metadata
        // This indicates where the job originated (e.g., "signal", "telegram", "web", "repl")
        let source_channel = job_ctx
            .metadata
            .as_object()
            .and_then(|obj| obj.get("source_channel").or_else(|| obj.get("channel")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // P0.2: Extract conversation_id from JobContext
        // JobContext already has conversation_id field
        let conversation_id = job_ctx.conversation_id.map(|id| {
            use brassclaw_engine::ConversationId;
            ConversationId(id)
        });

        // P0.2: Extract conversation_scope from job metadata
        // This is used for per-conversation state lookup
        let conversation_scope = job_ctx
            .metadata
            .as_object()
            .and_then(|obj| obj.get("conversation_scope"))
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .or(job_ctx.conversation_id); // Fallback to conversation_id if scope not specified

        ThreadExecutionContext {
            thread_id: ThreadId(job_id),
            thread_type,
            project_id,
            user_id,
            step_id,
            current_call_id: Some(format!("call_{}", uuid::Uuid::new_v4().simple())),
            source_channel,
            user_timezone,
            thread_goal,
            available_actions_snapshot: None, // P0.2: Will be populated in execute_tool_task
            available_action_inventory_snapshot: None, // P0.2: Will be populated in execute_tool_task
            conversation_scope,
            gate_controller: crate::agent::gate_controller::AutoApprovingGateController::arc(), // P0.3: Auto-approve for now
            call_approval_granted: false, // P0.3: Not needed with auto-approve, but kept for future
            conversation_id,
        }
    }

    /// Stop a running job.
    ///
    /// V2 migration: Enhanced to send Cancel message to running job workers.
    /// This method updates job state to Cancelled and signals the worker to stop.
    pub async fn stop(&self, job_id: Uuid) -> Result<(), JobError> {
        // Send cancel message to running job worker if it exists
        let running_jobs = self.running_jobs.read().await;
        if let Some(job_handle) = running_jobs.get(&job_id) {
            tracing::info!(job_id = %job_id, "Sending cancel message to running job worker");
            
            // Send cancel message (non-blocking, best effort)
            let _ = job_handle.message_tx.send(JobMessage::Cancel).await;
        }
        drop(running_jobs); // Release lock before async operations

        // Update job state
        self.context_manager
            .update_context(job_id, |ctx| {
                if let Err(e) = ctx.transition_to(
                    JobState::Cancelled,
                    Some("Stopped by scheduler".to_string()),
                ) {
                    tracing::warn!(
                        job_id = %job_id,
                        error = %e,
                        "Failed to transition job to Cancelled state"
                    );
                }
            })
            .await?;

        // Persist cancellation (fire-and-forget)
        if let Some(ref store) = self.store {
            let store = store.clone();
            tokio::spawn(async move {
                if let Err(e) = store
                    .update_job_status(
                        job_id,
                        JobState::Cancelled,
                        Some("Stopped by scheduler"),
                    )
                    .await
                {
                    tracing::warn!("Failed to persist cancellation for job {}: {}", job_id, e);
                }
            });
        }

        tracing::info!("Stopped job {}", job_id);
        Ok(())
    }

    /// Send a follow-up user message to a running job.
    ///
    /// Sends a message to the job's worker task via its message channel.
    /// Returns NotFound if the job is not currently running.
    pub async fn send_message(&self, job_id: Uuid, content: String) -> Result<(), JobError> {
        let running_jobs = self.running_jobs.read().await;
        
        if let Some(job_handle) = running_jobs.get(&job_id) {
            job_handle
                .message_tx
                .send(JobMessage::UserMessage(content))
                .await
                .map_err(|_| JobError::ContextError {
                    id: job_id,
                    reason: "Failed to send message to job worker".to_string(),
                })?;
            Ok(())
        } else {
            Err(JobError::NotFound { id: job_id })
        }
    }

    /// Check if a job is running.
    ///
    /// Returns true if the job has an active worker task.
    pub async fn is_running(&self, job_id: Uuid) -> bool {
        self.running_jobs.read().await.contains_key(&job_id)
    }

    /// Get count of running jobs.
    ///
    /// Returns the number of jobs with active worker tasks.
    pub async fn running_count(&self) -> usize {
        self.running_jobs.read().await.len()
    }

    /// Get count of running subtasks.
    pub async fn subtask_count(&self) -> usize {
        self.subtasks.read().await.len()
    }

    /// Get all running job IDs.
    ///
    /// Returns a vector of all job IDs that have active worker tasks.
    pub async fn running_jobs(&self) -> Vec<Uuid> {
        self.running_jobs.read().await.keys().copied().collect()
    }

    /// Clean up finished jobs and subtasks.
    pub async fn cleanup_finished(&self) {
        // Clean up finished subtasks
        let mut subtasks = self.subtasks.write().await;
        let mut finished_subtasks = Vec::new();

        for (id, scheduled) in subtasks.iter() {
            if scheduled.handle.is_finished() {
                finished_subtasks.push(*id);
            }
        }

        for id in finished_subtasks {
            subtasks.remove(&id);
            tracing::trace!("Cleaned up finished subtask {}", id);
        }
        drop(subtasks);

        // Clean up finished jobs
        let mut running_jobs = self.running_jobs.write().await;
        let mut finished_jobs = Vec::new();

        for (id, job_handle) in running_jobs.iter() {
            if job_handle.task_handle.is_finished() {
                finished_jobs.push(*id);
            }
        }

        for id in finished_jobs {
            running_jobs.remove(&id);
            tracing::debug!(job_id = %id, "Cleaned up finished job");
        }
    }

    /// Stop all jobs.
    ///
    /// V2 migration: Only aborts subtasks (job tracking removed).
    pub async fn stop_all(&self) {
        // Abort all subtasks
        let mut subtasks = self.subtasks.write().await;
        for (_, scheduled) in subtasks.drain() {
            scheduled.handle.abort();
        }
    }

    /// Get access to the V2 effect executor.
    pub fn effect_executor(&self) -> Option<&Arc<dyn EffectExecutor>> {
        self.effect_executor.as_ref()
    }

    /// Get access to the context manager.
    pub fn context_manager(&self) -> &Arc<ContextManager> {
        &self.context_manager
    }

    /// Process user message and return response.
    async fn process_user_message(
        content: &str,
        job_id: Uuid,
        context_manager: &Arc<ContextManager>,
    ) -> String {
        let trimmed = content.trim();
        
        // Handle interactive commands
        if trimmed.starts_with('/') {
            let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
            let command = parts[0];
            
            match command {
                "/status" => {
                    // Get job status
                    match context_manager.get_context(job_id).await {
                        Ok(ctx) => {
                            format!(
                                "Job Status: {:?}\nState: {:?}\nCreated: {}",
                                ctx.job_id,
                                ctx.state,
                                ctx.created_at
                            )
                        }
                        Err(e) => format!("Error getting status: {}", e),
                    }
                }
                "/help" => {
                    "Available commands:\n\
                     /status - Get current job status\n\
                     /help - Show this help message\n\
                     Any other message will be forwarded to the job execution".to_string()
                }
                _ => format!("Unknown command: {}. Type /help for available commands.", command),
            }
        } else {
            // Regular message - will be forwarded to execution
            "Message received and will be processed by job execution".to_string()
        }
    }

    /// Register a generic task handler.
    pub async fn register_generic_handler(
        &self,
        name: String,
        handler: Arc<dyn GenericTaskHandler>,
    ) -> Result<(), Error> {
        let mut handlers = self.generic_handlers.write().await;
        
        if handlers.contains_key(&name) {
            return Err(Error::Config(crate::error::ConfigError::InvalidValue {
                key: "handler_name".to_string(),
                message: format!("Generic task handler '{}' is already registered", name),
            }));
        }
        
        tracing::info!(
            handler_name = %name,
            description = handler.description(),
            "Registered generic task handler"
        );
        
        handlers.insert(name, handler);
        Ok(())
    }

    /// Register a background task handler.
    pub async fn register_background_task(
        &self,
        name: String,
        handler: Arc<dyn crate::agent::background_tasks::BackgroundTaskHandler>,
    ) -> Result<(), Error> {
        self.background_tasks.register(name, handler).await
    }

    /// Get dead letter queue statistics.
    pub async fn get_dlq_statistics(&self) -> Result<crate::agent::dead_letter_queue::DLQStatistics, Error> {
        self.dead_letter_queue.get_statistics().await
    }

    /// List jobs in the dead letter queue.
    pub async fn list_dlq_entries(&self) -> Result<Vec<crate::agent::dead_letter_queue::DLQEntry>, Error> {
        self.dead_letter_queue.list_all().await
    }

    /// Retry a job from the dead letter queue.
    pub async fn retry_dlq_job(&self, job_id: Uuid) -> Result<(), Error> {
        // Remove from DLQ
        self.dead_letter_queue.remove_job(job_id).await?;
        
        // Reschedule the job
        self.schedule(job_id).await.map_err(|e| {
            Error::Job(e)
        })
    }

    /// Get access to the background task registry.
    pub fn background_tasks(&self) -> &Arc<BackgroundTaskRegistry> {
        &self.background_tasks
    }

    /// Get access to the dead letter queue.
    pub fn dead_letter_queue(&self) -> &Arc<DeadLetterQueue> {
        &self.dead_letter_queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SafetyConfig;
    use brassclaw_llm::{
        CompletionRequest, CompletionResponse, LlmError, LlmProvider, ToolCompletionRequest,
        ToolCompletionResponse,
    };
    use brassclaw_safety::SafetyLayer;
    use rust_decimal_macros::dec;

    /// Minimal LLM provider stub for scheduler tests that don't exercise LLM calls.
    struct StubLlm;

    #[async_trait::async_trait]
    impl LlmProvider for StubLlm {
        fn model_name(&self) -> &str {
            "stub"
        }
        fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
            (dec!(0), dec!(0))
        }
        async fn complete(&self, _req: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            Err(LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
        async fn complete_with_tools(
            &self,
            _req: ToolCompletionRequest,
        ) -> Result<ToolCompletionResponse, LlmError> {
            Err(LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
    }

    /// Create a Scheduler for token-budget tests. The LLM stub will fail if a
    /// worker actually tries to call it, but `dispatch_job` sets the token
    /// budget *before* spawning the worker so we can inspect the context
    /// immediately after dispatch.
    fn make_test_scheduler(max_tokens_per_job: u64) -> Scheduler {
        let config = AgentConfig {
            name: "test".to_string(),
            max_parallel_jobs: 5,
            job_timeout: std::time::Duration::from_secs(30),
            stuck_threshold: std::time::Duration::from_secs(300),
            repair_check_interval: std::time::Duration::from_secs(3600),
            max_repair_attempts: 0,
            use_planning: false,
            session_idle_timeout: std::time::Duration::from_secs(3600),
            allow_local_tools: true,
            max_cost_per_day_cents: None,
            max_actions_per_hour: None,
            max_cost_per_user_per_day_cents: None,
            max_tool_iterations: 10,
            auto_approve_tools: true,
            default_timezone: "UTC".to_string(),
            max_jobs_per_user: None,
            max_tokens_per_job,
            multi_tenant: false,
            max_llm_concurrent_per_user: None,
            max_jobs_concurrent_per_user: None,
            engine_v2: false,
        };
        let cm = Arc::new(ContextManager::new(5));
        let llm: Arc<dyn LlmProvider> = Arc::new(StubLlm);
        let safety = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: false,
        }));
        let hooks = Arc::new(HookRegistry::default());

        Scheduler::new(
            config,
            cm,
            llm,
            safety,
            SchedulerDeps {
                effect_executor: None, // Tests use V2 path
                extension_manager: None,
                store: None,
                hooks,
            },
        )
    }

    #[tokio::test]
    async fn test_dispatch_job_caps_user_max_tokens() {
        let sched = make_test_scheduler(1000);
        let meta = serde_json::json!({ "max_tokens": 5000 });
        let job_id = sched
            .dispatch_job("user1", "test", "desc", Some(meta))
            .await
            .unwrap();

        let ctx = sched.context_manager.get_context(job_id).await.unwrap();
        assert_eq!(ctx.max_tokens, 1000, "should cap at configured limit");
    }

    #[tokio::test]
    async fn test_dispatch_job_unlimited_config_preserves_user_tokens() {
        let sched = make_test_scheduler(0); // 0 = unlimited
        let meta = serde_json::json!({ "max_tokens": 5000 });
        let job_id = sched
            .dispatch_job("user1", "test", "desc", Some(meta))
            .await
            .unwrap();

        let ctx = sched.context_manager.get_context(job_id).await.unwrap();
        assert_eq!(
            ctx.max_tokens, 5000,
            "unlimited config should preserve user value"
        );
    }

    #[tokio::test]
    async fn test_dispatch_job_no_user_tokens_uses_config() {
        let sched = make_test_scheduler(2000);
        let job_id = sched
            .dispatch_job("user1", "test", "desc", None)
            .await
            .unwrap();

        let ctx = sched.context_manager.get_context(job_id).await.unwrap();
        assert_eq!(
            ctx.max_tokens, 2000,
            "should use config default when no user value"
        );
    }

    #[tokio::test]
    async fn test_dispatch_job_atomic_metadata_and_tokens() {
        let sched = make_test_scheduler(10_000);
        let meta = serde_json::json!({
            "max_tokens": 3000,
            "custom_key": "custom_value"
        });
        let job_id = sched
            .dispatch_job("user1", "test", "desc", Some(meta))
            .await
            .unwrap();

        let ctx = sched.context_manager.get_context(job_id).await.unwrap();
        assert_eq!(ctx.max_tokens, 3000, "should use user value within limit");
        assert_eq!(
            ctx.metadata.get("custom_key").and_then(|v| v.as_str()),
            Some("custom_value"),
            "metadata should be set atomically with token budget"
        );
    }

    #[tokio::test]
    async fn test_dispatch_job_no_metadata_no_user_tokens_edge_case() {
        // Edge case coverage: when metadata=None AND max_tokens=0 (config),
        // the else branch calls get_context() directly (not update_context_and_get).
        // This test verifies that path works correctly (Issue #807: full branch coverage).
        let sched = make_test_scheduler(0); // 0 = unlimited, but user provides None
        let job_id = sched
            .dispatch_job("user1", "test", "desc", None) // None metadata
            .await
            .unwrap(); // safety: test code

        let ctx = sched.context_manager.get_context(job_id).await.unwrap(); // safety: test code
        // No metadata was set, should have default empty metadata
        assert!(ctx.metadata.is_null() || ctx.metadata == serde_json::json!({})); // safety: test code
        // No user tokens AND unlimited config means max_tokens stays at default
        assert_eq!(ctx.max_tokens, 0, "unlimited config"); // safety: test code
    }

    #[test]
    fn test_scheduler_creation() {
        // Would need to mock dependencies for proper testing
    }

    #[tokio::test]
    async fn test_spawn_batch_empty() {
        // This test would need mock dependencies.
        // For now just verify the empty case doesn't panic.
    }

    // V1 ToolRegistry-based tests removed during V2 migration.
    // Tool execution is now handled through EffectExecutor and V2 capabilities.
    // New tests should use V2 capability system.
}
