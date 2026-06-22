//! Dead Letter Queue (DLQ) for failed jobs.
//!
//! Provides persistent storage and retry scheduling for jobs that fail repeatedly.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::Error;

/// Entry in the dead letter queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DLQEntry {
    /// Job ID that failed.
    pub job_id: Uuid,
    /// Number of times this job has failed.
    pub failure_count: u32,
    /// Last error message.
    pub last_error: String,
    /// All error messages from failed attempts.
    pub error_history: Vec<String>,
    /// When the job first failed.
    pub first_failed_at: DateTime<Utc>,
    /// When the job last failed.
    pub last_failed_at: DateTime<Utc>,
    /// When to retry the job (if scheduled for retry).
    pub retry_after: Option<DateTime<Utc>>,
    /// Whether this job is permanently failed (no more retries).
    pub permanent_failure: bool,
    /// Original job metadata for context.
    pub job_metadata: serde_json::Value,
}

impl DLQEntry {
    /// Create a new DLQ entry for a failed job.
    pub fn new(job_id: Uuid, error: String, job_metadata: serde_json::Value) -> Self {
        let now = Utc::now();
        Self {
            job_id,
            failure_count: 1,
            last_error: error.clone(),
            error_history: vec![error],
            first_failed_at: now,
            last_failed_at: now,
            retry_after: None,
            permanent_failure: false,
            job_metadata,
        }
    }

    /// Record another failure for this job.
    pub fn record_failure(&mut self, error: String) {
        self.failure_count += 1;
        self.last_error = error.clone();
        self.error_history.push(error);
        self.last_failed_at = Utc::now();
    }

    /// Calculate next retry time using exponential backoff.
    pub fn calculate_retry_time(&self, base_delay: Duration, max_delay: Duration) -> DateTime<Utc> {
        let backoff_multiplier = 2u32.pow(self.failure_count.saturating_sub(1));
        let delay = base_delay * backoff_multiplier;
        let capped_delay = delay.min(max_delay);
        
        Utc::now() + chrono::Duration::from_std(capped_delay).unwrap_or(chrono::Duration::seconds(60))
    }

    /// Check if this entry is ready for retry.
    pub fn is_ready_for_retry(&self) -> bool {
        if self.permanent_failure {
            return false;
        }
        
        match self.retry_after {
            Some(retry_time) => Utc::now() >= retry_time,
            None => false,
        }
    }
}

/// Storage backend for the dead letter queue.
#[async_trait]
pub trait DLQStorage: Send + Sync {
    /// Add a job to the DLQ.
    async fn add_entry(&self, entry: DLQEntry) -> Result<(), Error>;
    
    /// Update an existing DLQ entry.
    async fn update_entry(&self, entry: DLQEntry) -> Result<(), Error>;
    
    /// Get a DLQ entry by job ID.
    async fn get_entry(&self, job_id: Uuid) -> Result<Option<DLQEntry>, Error>;
    
    /// Remove a job from the DLQ.
    async fn remove_entry(&self, job_id: Uuid) -> Result<(), Error>;
    
    /// List all entries in the DLQ.
    async fn list_entries(&self) -> Result<Vec<DLQEntry>, Error>;
    
    /// List entries ready for retry.
    async fn list_ready_for_retry(&self) -> Result<Vec<DLQEntry>, Error>;
    
    /// Get statistics about the DLQ.
    async fn get_statistics(&self) -> Result<DLQStatistics, Error>;
}

/// Statistics about the dead letter queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DLQStatistics {
    /// Total number of entries in the DLQ.
    pub total_entries: usize,
    /// Number of entries scheduled for retry.
    pub scheduled_retries: usize,
    /// Number of permanent failures.
    pub permanent_failures: usize,
    /// Number of entries ready for retry now.
    pub ready_for_retry: usize,
}

/// In-memory DLQ storage (for testing and simple deployments).
pub struct InMemoryDLQStorage {
    entries: RwLock<Vec<DLQEntry>>,
}

impl InMemoryDLQStorage {
    /// Create a new in-memory DLQ storage.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryDLQStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DLQStorage for InMemoryDLQStorage {
    async fn add_entry(&self, entry: DLQEntry) -> Result<(), Error> {
        let mut entries = self.entries.write().await;
        
        // Check if entry already exists
        if entries.iter().any(|e| e.job_id == entry.job_id) {
            return Err(Error::Config(crate::error::ConfigError::InvalidValue {
                key: "job_id".to_string(),
                message: format!("DLQ entry for job {} already exists", entry.job_id),
            }));
        }
        
        entries.push(entry);
        Ok(())
    }

    async fn update_entry(&self, entry: DLQEntry) -> Result<(), Error> {
        let mut entries = self.entries.write().await;
        
        if let Some(existing) = entries.iter_mut().find(|e| e.job_id == entry.job_id) {
            *existing = entry;
            Ok(())
        } else {
            Err(Error::Config(crate::error::ConfigError::InvalidValue {
                key: "job_id".to_string(),
                message: format!("DLQ entry for job {} not found", entry.job_id),
            }))
        }
    }

    async fn get_entry(&self, job_id: Uuid) -> Result<Option<DLQEntry>, Error> {
        let entries = self.entries.read().await;
        Ok(entries.iter().find(|e| e.job_id == job_id).cloned())
    }

    async fn remove_entry(&self, job_id: Uuid) -> Result<(), Error> {
        let mut entries = self.entries.write().await;
        entries.retain(|e| e.job_id != job_id);
        Ok(())
    }

    async fn list_entries(&self) -> Result<Vec<DLQEntry>, Error> {
        let entries = self.entries.read().await;
        Ok(entries.clone())
    }

    async fn list_ready_for_retry(&self) -> Result<Vec<DLQEntry>, Error> {
        let entries = self.entries.read().await;
        Ok(entries
            .iter()
            .filter(|e| e.is_ready_for_retry())
            .cloned()
            .collect())
    }

    async fn get_statistics(&self) -> Result<DLQStatistics, Error> {
        let entries = self.entries.read().await;
        
        let total_entries = entries.len();
        let scheduled_retries = entries.iter().filter(|e| e.retry_after.is_some() && !e.permanent_failure).count();
        let permanent_failures = entries.iter().filter(|e| e.permanent_failure).count();
        let ready_for_retry = entries.iter().filter(|e| e.is_ready_for_retry()).count();
        
        Ok(DLQStatistics {
            total_entries,
            scheduled_retries,
            permanent_failures,
            ready_for_retry,
        })
    }
}

/// Configuration for the dead letter queue.
#[derive(Debug, Clone)]
pub struct DLQConfig {
    /// Maximum number of retry attempts before permanent failure.
    pub max_retries: u32,
    /// Base delay for exponential backoff (first retry).
    pub base_retry_delay: Duration,
    /// Maximum delay between retries.
    pub max_retry_delay: Duration,
    /// Whether to enable automatic retry scheduling.
    pub auto_retry: bool,
}

impl Default for DLQConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_retry_delay: Duration::from_secs(60),      // 1 minute
            max_retry_delay: Duration::from_secs(3600),     // 1 hour
            auto_retry: true,
        }
    }
}

/// Dead Letter Queue manager.
pub struct DeadLetterQueue {
    storage: Arc<dyn DLQStorage>,
    config: DLQConfig,
}

impl DeadLetterQueue {
    /// Create a new DLQ with the given storage backend.
    pub fn new(storage: Arc<dyn DLQStorage>, config: DLQConfig) -> Self {
        Self { storage, config }
    }

    /// Create a DLQ with in-memory storage (for testing).
    pub fn new_in_memory(config: DLQConfig) -> Self {
        Self::new(Arc::new(InMemoryDLQStorage::new()), config)
    }

    /// Add a failed job to the DLQ.
    pub async fn add_failed_job(
        &self,
        job_id: Uuid,
        error: String,
        job_metadata: serde_json::Value,
    ) -> Result<(), Error> {
        tracing::warn!(
            job_id = %job_id,
            error = %error,
            "Adding job to dead letter queue"
        );

        // Check if job already exists in DLQ
        if let Some(mut entry) = self.storage.get_entry(job_id).await? {
            // Update existing entry
            entry.record_failure(error);
            
            if entry.failure_count >= self.config.max_retries {
                // Mark as permanent failure
                entry.permanent_failure = true;
                entry.retry_after = None;
                
                tracing::error!(
                    job_id = %job_id,
                    failure_count = entry.failure_count,
                    "Job marked as permanent failure after max retries"
                );
            } else if self.config.auto_retry {
                // Schedule retry with exponential backoff
                entry.retry_after = Some(entry.calculate_retry_time(
                    self.config.base_retry_delay,
                    self.config.max_retry_delay,
                ));
                
                tracing::info!(
                    job_id = %job_id,
                    failure_count = entry.failure_count,
                    retry_after = ?entry.retry_after,
                    "Scheduled job for retry"
                );
            }
            
            self.storage.update_entry(entry).await?;
        } else {
            // Create new entry
            let mut entry = DLQEntry::new(job_id, error, job_metadata);
            
            if self.config.auto_retry && entry.failure_count < self.config.max_retries {
                entry.retry_after = Some(entry.calculate_retry_time(
                    self.config.base_retry_delay,
                    self.config.max_retry_delay,
                ));
            }
            
            self.storage.add_entry(entry).await?;
        }

        Ok(())
    }

    /// Remove a job from the DLQ (after successful retry or manual intervention).
    pub async fn remove_job(&self, job_id: Uuid) -> Result<(), Error> {
        tracing::info!(job_id = %job_id, "Removing job from dead letter queue");
        self.storage.remove_entry(job_id).await
    }

    /// Get a specific DLQ entry.
    pub async fn get_entry(&self, job_id: Uuid) -> Result<Option<DLQEntry>, Error> {
        self.storage.get_entry(job_id).await
    }

    /// List all entries in the DLQ.
    pub async fn list_all(&self) -> Result<Vec<DLQEntry>, Error> {
        self.storage.list_entries().await
    }

    /// List entries ready for retry.
    pub async fn list_ready_for_retry(&self) -> Result<Vec<DLQEntry>, Error> {
        self.storage.list_ready_for_retry().await
    }

    /// Get DLQ statistics.
    pub async fn get_statistics(&self) -> Result<DLQStatistics, Error> {
        self.storage.get_statistics().await
    }

    /// Manually schedule a job for retry.
    pub async fn schedule_retry(&self, job_id: Uuid, retry_after: DateTime<Utc>) -> Result<(), Error> {
        if let Some(mut entry) = self.storage.get_entry(job_id).await? {
            if entry.permanent_failure {
                return Err(Error::Config(crate::error::ConfigError::InvalidValue {
                    key: "job_id".to_string(),
                    message: format!("Cannot retry job {} marked as permanent failure", job_id),
                }));
            }
            
            entry.retry_after = Some(retry_after);
            self.storage.update_entry(entry).await?;
            
            tracing::info!(
                job_id = %job_id,
                retry_after = %retry_after,
                "Manually scheduled job for retry"
            );
            
            Ok(())
        } else {
            Err(Error::Config(crate::error::ConfigError::InvalidValue {
                key: "job_id".to_string(),
                message: format!("Job {} not found in DLQ", job_id),
            }))
        }
    }

    /// Mark a job as permanently failed (no more retries).
    pub async fn mark_permanent_failure(&self, job_id: Uuid) -> Result<(), Error> {
        if let Some(mut entry) = self.storage.get_entry(job_id).await? {
            entry.permanent_failure = true;
            entry.retry_after = None;
            self.storage.update_entry(entry).await?;
            
            tracing::warn!(
                job_id = %job_id,
                "Marked job as permanent failure"
            );
            
            Ok(())
        } else {
            Err(Error::Config(crate::error::ConfigError::InvalidValue {
                key: "job_id".to_string(),
                message: format!("Job {} not found in DLQ", job_id),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dlq_entry_creation() {
        let job_id = Uuid::new_v4();
        let entry = DLQEntry::new(
            job_id,
            "Test error".to_string(),
            serde_json::json!({"test": true}),
        );
        
        assert_eq!(entry.job_id, job_id);
        assert_eq!(entry.failure_count, 1);
        assert_eq!(entry.last_error, "Test error");
        assert!(!entry.permanent_failure);
    }

    #[tokio::test]
    async fn test_dlq_add_and_retrieve() {
        let dlq = DeadLetterQueue::new_in_memory(DLQConfig::default());
        let job_id = Uuid::new_v4();
        
        dlq.add_failed_job(
            job_id,
            "Test error".to_string(),
            serde_json::json!({}),
        ).await.unwrap();
        
        let entry = dlq.get_entry(job_id).await.unwrap().unwrap();
        assert_eq!(entry.job_id, job_id);
        assert_eq!(entry.failure_count, 1);
    }

    #[tokio::test]
    async fn test_dlq_retry_scheduling() {
        let config = DLQConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_secs(10),
            max_retry_delay: Duration::from_secs(60),
            auto_retry: true,
        };
        
        let dlq = DeadLetterQueue::new_in_memory(config);
        let job_id = Uuid::new_v4();
        
        // First failure - should schedule retry
        dlq.add_failed_job(job_id, "Error 1".to_string(), serde_json::json!({})).await.unwrap();
        let entry = dlq.get_entry(job_id).await.unwrap().unwrap();
        assert!(entry.retry_after.is_some());
        assert!(!entry.permanent_failure);
        
        // Second failure
        dlq.add_failed_job(job_id, "Error 2".to_string(), serde_json::json!({})).await.unwrap();
        let entry = dlq.get_entry(job_id).await.unwrap().unwrap();
        assert_eq!(entry.failure_count, 2);
        
        // Third failure - should mark as permanent
        dlq.add_failed_job(job_id, "Error 3".to_string(), serde_json::json!({})).await.unwrap();
        let entry = dlq.get_entry(job_id).await.unwrap().unwrap();
        assert_eq!(entry.failure_count, 3);
        assert!(entry.permanent_failure);
        assert!(entry.retry_after.is_none());
    }

    #[tokio::test]
    async fn test_dlq_statistics() {
        let dlq = DeadLetterQueue::new_in_memory(DLQConfig::default());
        
        // Add some entries
        for i in 0..5 {
            let job_id = Uuid::new_v4();
            dlq.add_failed_job(
                job_id,
                format!("Error {}", i),
                serde_json::json!({}),
            ).await.unwrap();
        }
        
        let stats = dlq.get_statistics().await.unwrap();
        assert_eq!(stats.total_entries, 5);
    }
}

// Made with Bob
