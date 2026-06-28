//! Test utilities: stub LLM, stub channel, and test DB helpers.

pub mod credentials;

// `StubLlm`, `StubErrorKind`, and `fault_injection` live in `brassclaw_llm`
// (the natural home for the trait they implement). Re-exported under the
// existing `crate::testing::*` paths so existing test imports keep working.
pub use brassclaw_llm::testing::{StubErrorKind, StubLlm, fault_injection};

use std::sync::Arc;
use std::sync::Mutex;

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tokio::sync::{Mutex as AsyncMutex, mpsc};

use crate::channels::{Channel, IncomingMessage, MessageStream, OutgoingResponse, StatusUpdate};
use crate::db::Database;
use crate::error::ChannelError;

/// Create a libSQL-backed test database in a temporary directory.
///
/// Returns the database and a `TempDir` guard — the database file is
/// deleted when the guard is dropped.
#[cfg(feature = "libsql")]
pub async fn test_db() -> (Arc<dyn Database>, tempfile::TempDir) {
    use crate::db::libsql::LibSqlBackend;

    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("test.db");
    let backend = LibSqlBackend::new_local(&path)
        .await
        .expect("failed to create test LibSqlBackend");
    backend
        .run_migrations()
        .await
        .expect("failed to run migrations");
    (Arc::new(backend) as Arc<dyn Database>, dir)
}

/// A configurable channel stub for tests.
///
/// Supports:
/// - Message injection via the returned `mpsc::Sender`
/// - Response capture for assertion
/// - Status update capture
/// - Configurable health check failure
///
/// # Usage
///
/// ```rust,no_run
/// let (channel, sender) = StubChannel::new("test");
/// sender.send(IncomingMessage::new("test", "user1", "hello")).await.unwrap();
/// // ... run agent logic that calls channel.respond() ...
/// let responses = channel.captured_responses();
/// ```
pub struct StubChannel {
    name: String,
    rx: tokio::sync::Mutex<Option<mpsc::Receiver<IncomingMessage>>>,
    responses: Arc<Mutex<Vec<(IncomingMessage, OutgoingResponse)>>>,
    statuses: Arc<Mutex<Vec<StatusUpdate>>>,
    healthy: AtomicBool,
}

impl StubChannel {
    /// Create a new stub channel and its message sender.
    ///
    /// The sender is used by tests to inject messages into the channel's stream.
    /// The channel captures all responses and status updates for later assertion.
    pub fn new(name: impl Into<String>) -> (Self, mpsc::Sender<IncomingMessage>) {
        let (tx, rx) = mpsc::channel(64);
        let channel = Self {
            name: name.into(),
            rx: tokio::sync::Mutex::new(Some(rx)),
            responses: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(Vec::new())),
            healthy: AtomicBool::new(true),
        };
        (channel, tx)
    }

    /// Get all captured (message, response) pairs.
    pub fn captured_responses(&self) -> Vec<(IncomingMessage, OutgoingResponse)> {
        self.responses.lock().expect("poisoned").clone()
    }

    /// Get a shared handle to the response capture list.
    ///
    /// Call this *before* moving the channel into a `ChannelManager`,
    /// since `add()` takes ownership.
    pub fn captured_responses_handle(
        &self,
    ) -> Arc<Mutex<Vec<(IncomingMessage, OutgoingResponse)>>> {
        Arc::clone(&self.responses)
    }

    /// Get all captured status updates.
    pub fn captured_statuses(&self) -> Vec<StatusUpdate> {
        self.statuses.lock().expect("poisoned").clone()
    }

    /// Get a shared handle to the status capture list.
    pub fn captured_statuses_handle(&self) -> Arc<Mutex<Vec<StatusUpdate>>> {
        Arc::clone(&self.statuses)
    }

    /// Set whether `health_check()` succeeds or fails.
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Relaxed);
    }
}

#[async_trait]
impl Channel for StubChannel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&self) -> Result<MessageStream, ChannelError> {
        let rx = self
            .rx
            .lock()
            .await
            .take()
            .ok_or_else(|| ChannelError::StartupFailed {
                name: self.name.clone(),
                reason: "start() already called".to_string(),
            })?;
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    async fn respond(
        &self,
        msg: &IncomingMessage,
        response: OutgoingResponse,
    ) -> Result<(), ChannelError> {
        self.responses
            .lock()
            .expect("poisoned")
            .push((msg.clone(), response));
        Ok(())
    }

    async fn send_status(
        &self,
        status: StatusUpdate,
        _metadata: &serde_json::Value,
    ) -> Result<(), ChannelError> {
        self.statuses.lock().expect("poisoned").push(status);
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ChannelError> {
        if self.healthy.load(Ordering::Relaxed) {
            Ok(())
        } else {
            Err(ChannelError::HealthCheckFailed {
                name: self.name.clone(),
            })
        }
    }
}

/// Captured broadcast deliveries keyed by the target user or chat identifier.
pub type BroadcastCapture = Arc<AsyncMutex<Vec<(String, OutgoingResponse)>>>;

/// A lightweight channel double that only records `broadcast()` traffic.
///
/// This is useful for unit tests that need to assert message routing without
/// spinning up a full interactive channel harness.
pub struct RecordingBroadcastChannel {
    name: &'static str,
    captures: BroadcastCapture,
}

impl RecordingBroadcastChannel {
    pub fn new(name: &'static str) -> (Self, BroadcastCapture) {
        let captures = Arc::new(AsyncMutex::new(Vec::new()));
        (
            Self {
                name,
                captures: Arc::clone(&captures),
            },
            captures,
        )
    }
}

#[async_trait]
impl Channel for RecordingBroadcastChannel {
    fn name(&self) -> &str {
        self.name
    }

    async fn start(&self) -> Result<MessageStream, ChannelError> {
        let (_tx, rx) = mpsc::channel::<IncomingMessage>(1);
        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    async fn respond(
        &self,
        _msg: &IncomingMessage,
        _response: OutgoingResponse,
    ) -> Result<(), ChannelError> {
        Ok(())
    }

    async fn send_status(
        &self,
        _status: StatusUpdate,
        _metadata: &serde_json::Value,
    ) -> Result<(), ChannelError> {
        Ok(())
    }

    async fn broadcast(
        &self,
        user_id: &str,
        response: OutgoingResponse,
    ) -> Result<(), ChannelError> {
        self.captures
            .lock()
            .await
            .push((user_id.to_string(), response));
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ChannelError> {
        Ok(())
    }
}
