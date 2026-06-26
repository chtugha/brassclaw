//! Background task execution system.
//!
//! Provides a registry-based system for executing background tasks with progress tracking,
//! cancellation support, and extensibility.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::Error;

/// Progress reporter for long-running background tasks.
#[derive(Clone)]
pub struct ProgressReporter {
    job_id: Uuid,
    progress: Arc<RwLock<TaskProgress>>,
}

impl ProgressReporter {
    /// Create a new progress reporter.
    pub fn new(job_id: Uuid) -> Self {
        Self {
            job_id,
            progress: Arc::new(RwLock::new(TaskProgress::default())),
        }
    }

    /// Update progress percentage (0-100).
    pub async fn update_progress(&self, percent: u8, message: Option<String>) {
        let mut progress = self.progress.write().await;
        progress.percent = percent.min(100);
        if let Some(msg) = message {
            progress.message = Some(msg);
        }
        progress.last_updated = Instant::now();
    }

    /// Mark a milestone in task execution.
    pub async fn add_milestone(&self, milestone: String) {
        let mut progress = self.progress.write().await;
        progress.milestones.push(milestone);
        progress.last_updated = Instant::now();
    }

    /// Get current progress snapshot.
    pub async fn get_progress(&self) -> TaskProgress {
        self.progress.read().await.clone()
    }

    /// Get job ID.
    pub fn job_id(&self) -> Uuid {
        self.job_id
    }
}

/// Progress information for a background task.
#[derive(Debug, Clone)]
pub struct TaskProgress {
    /// Progress percentage (0-100).
    pub percent: u8,
    /// Optional progress message.
    pub message: Option<String>,
    /// Milestones reached during execution.
    pub milestones: Vec<String>,
    /// Last time progress was updated.
    pub last_updated: Instant,
}

impl Default for TaskProgress {
    fn default() -> Self {
        Self {
            percent: 0,
            message: None,
            milestones: Vec::new(),
            last_updated: Instant::now(),
        }
    }
}

impl TaskProgress {
    /// Convert to JSON value for storage.
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "percent": self.percent,
            "message": self.message,
            "milestones": self.milestones,
            "last_updated_ms": self.last_updated.elapsed().as_millis(),
        })
    }
}

/// Context provided to background task handlers.
pub struct TaskContext {
    /// Job ID for this task execution.
    pub job_id: Uuid,
    /// Task-specific parameters.
    pub params: Value,
    /// Progress reporter for updating task status.
    pub progress: ProgressReporter,
}

/// Handler for a specific background task type.
#[async_trait]
pub trait BackgroundTaskHandler: Send + Sync {
    /// Execute the background task.
    ///
    /// Returns the task result as JSON, or an error if execution fails.
    async fn execute(&self, context: TaskContext) -> Result<Value, Error>;

    /// Get a human-readable description of this task type.
    fn description(&self) -> &str;
}

/// Registry of background task handlers.
pub struct BackgroundTaskRegistry {
    handlers: RwLock<HashMap<String, Arc<dyn BackgroundTaskHandler>>>,
}

impl BackgroundTaskRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Create a registry pre-populated with the built-in task handlers.
    ///
    /// This is a synchronous constructor — it avoids the async `register` path
    /// so it can be called from synchronous contexts (e.g., `Scheduler::new`).
    pub fn with_defaults() -> Self {
        let mut handlers: HashMap<String, Arc<dyn BackgroundTaskHandler>> = HashMap::new();
        handlers.insert("data_processing".to_string(), Arc::new(DataProcessingTask));
        handlers.insert("maintenance".to_string(), Arc::new(MaintenanceTask));
        handlers.insert("report_generation".to_string(), Arc::new(ReportGenerationTask));
        Self {
            handlers: RwLock::new(handlers),
        }
    }

    /// Register a background task handler.
    pub async fn register(
        &self,
        task_name: String,
        handler: Arc<dyn BackgroundTaskHandler>,
    ) -> Result<(), Error> {
        let mut handlers = self.handlers.write().await;
        
        if handlers.contains_key(&task_name) {
            return Err(Error::Config(crate::error::ConfigError::InvalidValue {
                key: "task_name".to_string(),
                message: format!("Background task '{}' is already registered", task_name),
            }));
        }

        tracing::info!(
            task_name = %task_name,
            description = handler.description(),
            "Registered background task handler"
        );

        handlers.insert(task_name, handler);
        Ok(())
    }

    /// Execute a background task by name.
    pub async fn execute(
        &self,
        job_id: Uuid,
        task_name: &str,
        params: Value,
    ) -> Result<Value, Error> {
        let handlers = self.handlers.read().await;
        
        let handler = handlers.get(task_name).ok_or_else(|| {
            Error::Config(crate::error::ConfigError::InvalidValue {
                key: "task_name".to_string(),
                message: format!(
                    "Unknown background task: '{}'. Available tasks: {}",
                    task_name,
                    handlers.keys().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
                ),
            })
        })?;

        let progress = ProgressReporter::new(job_id);
        let context = TaskContext {
            job_id,
            params,
            progress: progress.clone(),
        };

        tracing::info!(
            job_id = %job_id,
            task_name = %task_name,
            description = handler.description(),
            "Executing background task"
        );

        let start = Instant::now();
        let result = handler.execute(context).await?;
        let duration = start.elapsed();

        tracing::info!(
            job_id = %job_id,
            task_name = %task_name,
            duration_ms = duration.as_millis(),
            "Background task completed"
        );

        Ok(result)
    }

    /// List all registered task names.
    pub async fn list_tasks(&self) -> Vec<String> {
        let handlers = self.handlers.read().await;
        handlers.keys().cloned().collect()
    }

    /// Get description for a task.
    pub async fn get_description(&self, task_name: &str) -> Option<String> {
        let handlers = self.handlers.read().await;
        handlers.get(task_name).map(|h| h.description().to_string())
    }
}

impl Default for BackgroundTaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Built-in background task handlers

/// Data processing task handler.
pub struct DataProcessingTask;

#[async_trait]
impl BackgroundTaskHandler for DataProcessingTask {
    async fn execute(&self, context: TaskContext) -> Result<Value, Error> {
        context.progress.update_progress(0, Some("Starting data processing".to_string())).await;
        
        // Extract processing parameters
        let batch_size = context.params.get("batch_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(100);
        
        let total_items = context.params.get("total_items")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        context.progress.add_milestone("Initialized processing".to_string()).await;

        // Simulate batch processing
        let mut processed = 0u64;
        while processed < total_items {
            let batch_end = (processed + batch_size).min(total_items);
            
            // Simulate processing delay
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            processed = batch_end;
            let percent = ((processed as f64 / total_items as f64) * 100.0) as u8;
            
            context.progress.update_progress(
                percent,
                Some(format!("Processed {}/{} items", processed, total_items))
            ).await;
        }

        context.progress.add_milestone("Processing completed".to_string()).await;
        context.progress.update_progress(100, Some("Done".to_string())).await;

        Ok(serde_json::json!({
            "status": "completed",
            "items_processed": processed,
            "batch_size": batch_size,
        }))
    }

    fn description(&self) -> &str {
        "Process data in batches with progress tracking"
    }
}

/// Scheduled maintenance task handler.
pub struct MaintenanceTask;

#[async_trait]
impl BackgroundTaskHandler for MaintenanceTask {
    async fn execute(&self, context: TaskContext) -> Result<Value, Error> {
        context.progress.update_progress(0, Some("Starting maintenance".to_string())).await;
        
        let maintenance_type = context.params.get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        context.progress.add_milestone(format!("Running {} maintenance", maintenance_type)).await;

        // Simulate maintenance operations
        let steps = vec![
            "Cleaning temporary files",
            "Optimizing database",
            "Checking system health",
            "Updating caches",
        ];

        for (i, step) in steps.iter().enumerate() {
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            let percent = ((i + 1) as f64 / steps.len() as f64 * 100.0) as u8;
            context.progress.update_progress(percent, Some(step.to_string())).await;
            context.progress.add_milestone(format!("Completed: {}", step)).await;
        }

        context.progress.update_progress(100, Some("Maintenance complete".to_string())).await;

        Ok(serde_json::json!({
            "status": "completed",
            "maintenance_type": maintenance_type,
            "steps_completed": steps.len(),
        }))
    }

    fn description(&self) -> &str {
        "Perform scheduled system maintenance tasks"
    }
}

/// Report generation task handler.
pub struct ReportGenerationTask;

#[async_trait]
impl BackgroundTaskHandler for ReportGenerationTask {
    async fn execute(&self, context: TaskContext) -> Result<Value, Error> {
        context.progress.update_progress(0, Some("Initializing report generation".to_string())).await;
        
        let report_type = context.params.get("report_type")
            .and_then(|v| v.as_str())
            .unwrap_or("summary");

        context.progress.add_milestone("Gathering data".to_string()).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        context.progress.update_progress(25, Some("Data gathered".to_string())).await;

        context.progress.add_milestone("Analyzing data".to_string()).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        context.progress.update_progress(50, Some("Analysis complete".to_string())).await;

        context.progress.add_milestone("Formatting report".to_string()).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        context.progress.update_progress(75, Some("Report formatted".to_string())).await;

        context.progress.add_milestone("Finalizing report".to_string()).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        context.progress.update_progress(100, Some("Report ready".to_string())).await;

        Ok(serde_json::json!({
            "status": "completed",
            "report_type": report_type,
            "report_id": Uuid::new_v4().to_string(),
            "pages": 42,
        }))
    }

    fn description(&self) -> &str {
        "Generate reports with data analysis"
    }
}

/// Create a registry with built-in task handlers.
pub async fn create_default_registry() -> BackgroundTaskRegistry {
    let registry = BackgroundTaskRegistry::new();
    
    // Register built-in handlers
    let _ = registry.register(
        "data_processing".to_string(),
        Arc::new(DataProcessingTask),
    ).await;
    
    let _ = registry.register(
        "maintenance".to_string(),
        Arc::new(MaintenanceTask),
    ).await;
    
    let _ = registry.register(
        "report_generation".to_string(),
        Arc::new(ReportGenerationTask),
    ).await;

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_reporter() {
        let reporter = ProgressReporter::new(Uuid::new_v4());
        
        reporter.update_progress(50, Some("Half done".to_string())).await;
        let progress = reporter.get_progress().await;
        
        assert_eq!(progress.percent, 50);
        assert_eq!(progress.message, Some("Half done".to_string()));
    }

    #[tokio::test]
    async fn test_registry_registration() {
        let registry = BackgroundTaskRegistry::new();
        let handler = Arc::new(DataProcessingTask);
        
        registry.register("test_task".to_string(), handler).await.unwrap();
        
        let tasks = registry.list_tasks().await;
        assert!(tasks.contains(&"test_task".to_string()));
    }

    #[tokio::test]
    async fn test_default_registry() {
        let registry = create_default_registry().await;
        let tasks = registry.list_tasks().await;
        
        assert!(tasks.contains(&"data_processing".to_string()));
        assert!(tasks.contains(&"maintenance".to_string()));
        assert!(tasks.contains(&"report_generation".to_string()));
    }
}

// Made with Bob
