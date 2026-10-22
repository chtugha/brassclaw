//! Postgres-backed [`SessionThreadService`] implementation.
//!
//! Each thread's full state (thread record, messages, summary artifacts,
//! and inbound idempotency records) is stored as a JSONB blob in
//! `brassclaw_session_threads.metadata` keyed by `(tenant_id, thread_id)`.
//! A `version` counter provides optimistic CAS for concurrent mutations.
//!
//! Inbound idempotency receipts live in that same snapshot. Replay selects
//! matching receipt candidates in the exact canonical scope and checks the
//! stored identities without loading unrelated transcripts into the service.
//!
//! Cross-tenant isolation: every query is scoped by `tenant_id`.

mod admission;
mod replay;
mod submission_transaction;

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::ThreadId;
use brassclaw_pg::PgPool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::summary_artifacts::find_overlapping_summary;
use crate::{
    AcceptInboundMessageRequest, AcceptedInboundMessage, AcceptedInboundMessageReplay,
    AppendAssistantDraftRequest, AppendCapabilityDisplayPreviewRequest,
    AppendToolResultReferenceRequest, ContextMessage, ContextMessages, ContextWindow,
    CreateSummaryArtifactRequest, EnsureThreadRequest, ListThreadsForScopeRequest,
    ListThreadsForScopeResponse, LoadContextMessagesRequest, LoadContextWindowRequest,
    MessageContent, MessageKind, MessageStatus, RedactMessageRequest,
    ReplayAcceptedInboundMessageRequest, SessionThreadError, SessionThreadRecord,
    SessionThreadService, SummaryArtifact, SummaryModelContextPolicy, ThreadGoal, ThreadHistory,
    ThreadHistoryRequest, ThreadMessageId, ThreadMessageRecord, ThreadScope,
    ToolResultReferenceEnvelope, UpdateAssistantDraftRequest, UpdateThreadGoalRequest,
    UpdateToolResultReferenceRequest,
};

fn map_pool(_error: deadpool_postgres::PoolError) -> SessionThreadError {
    SessionThreadError::Backend("thread database pool unavailable".into())
}

fn map_pg(error: tokio_postgres::Error) -> SessionThreadError {
    let reason = match error.code() {
        Some(code) => format!("thread database error (SQLSTATE {})", code.code()),
        None => "thread database transport/protocol failure".into(),
    };
    SessionThreadError::Backend(reason)
}

fn map_json(error: serde_json::Error) -> SessionThreadError {
    SessionThreadError::Backend(format!(
        "thread snapshot encoding error ({:?})",
        error.classify()
    ))
}

// ---------------------------------------------------------------------------
// On-disk snapshot structures
// ---------------------------------------------------------------------------

/// Full per-thread snapshot stored in `brassclaw_session_threads.metadata`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct ThreadSnapshot {
    #[serde(default)]
    closed: bool,
    record: Option<SessionThreadRecord>,
    #[serde(serialize_with = "crate::stored_message::serialize_messages")]
    messages: Vec<ThreadMessageRecord>,
    summary_artifacts: Vec<SummaryArtifact>,
    next_sequence: u64,
    /// Idempotency records keyed within this thread.
    inbound_idempotency: Vec<InboundIdempotencyEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InboundIdempotencyEntry {
    scope_tenant_id: String,
    scope_agent_id: String,
    scope_owner_user_id: Option<String>,
    source_binding_id: String,
    external_event_id: String,
    thread_id: String,
    message_id: String,
}

// ---------------------------------------------------------------------------
// PgSessionThreadService
// ---------------------------------------------------------------------------

/// Postgres-backed [`SessionThreadService`].
pub struct PgSessionThreadService {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgSessionThreadService {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    // ------------------------------------------------------------------
    // Snapshot I/O
    // ------------------------------------------------------------------

    async fn read_snapshot(
        &self,
        thread_id: &ThreadId,
    ) -> Result<(ThreadSnapshot, i64), SessionThreadError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT metadata, version FROM brassclaw_session_threads \
                 WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
                &[&thread_id.as_str(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok((ThreadSnapshot::default(), 0)),
            Some(r) => {
                let payload: Value = r.get(0);
                let version: i64 = r.get(1);
                let snapshot = self.decode_snapshot(thread_id, payload, version)?;
                Ok((snapshot, version))
            }
        }
    }

    fn decode_snapshot(
        &self,
        thread_id: &ThreadId,
        payload: Value,
        version: i64,
    ) -> Result<ThreadSnapshot, SessionThreadError> {
        if version < 0 {
            return Err(SessionThreadError::Backend(
                "invalid thread snapshot version".into(),
            ));
        }
        let snapshot: ThreadSnapshot = serde_json::from_value(payload).map_err(map_json)?;
        if snapshot.record.as_ref().is_some_and(|record| {
            record.thread_id != *thread_id
                || record.scope.tenant_id.as_str() != self.tenant_id.as_str()
        }) {
            return Err(SessionThreadError::Backend(
                "thread snapshot identity mismatch".into(),
            ));
        }
        Ok(snapshot)
    }

    /// Write snapshot back with CAS on `version`. Returns `Ok(true)` on success,
    /// `Ok(false)` on version mismatch.
    async fn write_snapshot(
        &self,
        snapshot: &ThreadSnapshot,
        expected_version: i64,
    ) -> Result<bool, SessionThreadError> {
        let record = snapshot
            .record
            .as_ref()
            .ok_or_else(|| SessionThreadError::Backend("snapshot has no record".to_string()))?;
        if record.scope.tenant_id.as_str() != self.tenant_id.as_str() {
            return Err(SessionThreadError::ThreadScopeMismatch {
                thread_id: record.thread_id.clone(),
            });
        }
        let thread_id = record.thread_id.as_str().to_string();
        let user_id = record
            .scope
            .owner_user_id
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_else(|| brassclaw_host_api::SYSTEM_RESERVED_ID.to_string());
        let agent_id = record.scope.agent_id.to_string();
        let project_id = record.scope.project_id.as_ref().map(|p| p.to_string());
        let created_by = &record.created_by_actor_id;
        let title = record.title.as_deref();
        let next_version = expected_version.checked_add(1).ok_or_else(|| {
            SessionThreadError::Backend("thread snapshot version exhausted".into())
        })?;
        let metadata_val = serde_json::to_value(snapshot).map_err(map_json)?;

        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .execute(
                "INSERT INTO brassclaw_session_threads \
                 (id, tenant_id, user_id, agent_id, project_id, \
                  created_by_actor_id, title, metadata, version) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1) \
                 ON CONFLICT (id) DO UPDATE \
                 SET user_id = excluded.user_id, \
                     agent_id = excluded.agent_id, \
                     project_id = excluded.project_id, \
                     title = excluded.title, \
                     metadata = excluded.metadata, \
                     version = $9, \
                     updated_at = now() \
                 WHERE brassclaw_session_threads.version = $10 \
                   AND brassclaw_session_threads.tenant_id = $2 \
                   AND brassclaw_session_threads.user_id = excluded.user_id \
                   AND brassclaw_session_threads.agent_id = excluded.agent_id \
                   AND brassclaw_session_threads.project_id IS NOT DISTINCT FROM excluded.project_id \
                   AND brassclaw_session_threads.deleted_at IS NULL",
                &[
                    &thread_id,
                    &self.tenant_id,
                    &user_id,
                    &agent_id,
                    &project_id,
                    created_by,
                    &title,
                    &metadata_val,
                    &next_version,
                    &expected_version,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(rows > 0)
    }

    // ------------------------------------------------------------------
    // CAS apply loop
    // ------------------------------------------------------------------

    async fn apply<T, A, Fut>(
        &self,
        thread_id: &ThreadId,
        mut apply: A,
    ) -> Result<T, SessionThreadError>
    where
        A: FnMut(ThreadSnapshot) -> Fut,
        Fut: std::future::Future<Output = Result<(T, ThreadSnapshot), SessionThreadError>>,
    {
        self.apply_optional(thread_id, move |snapshot| {
            let mutation = apply(snapshot);
            async move {
                let (outcome, snapshot) = mutation.await?;
                Ok((outcome, Some(snapshot)))
            }
        })
        .await
    }

    /// Exact metadata acknowledgements may require no write. In that case do
    /// not serialize/rewrite the full transcript or advance its CAS generation.
    async fn apply_optional<T, A, Fut>(
        &self,
        thread_id: &ThreadId,
        mut apply: A,
    ) -> Result<T, SessionThreadError>
    where
        A: FnMut(ThreadSnapshot) -> Fut,
        Fut: std::future::Future<Output = Result<(T, Option<ThreadSnapshot>), SessionThreadError>>,
    {
        for _ in 0..12usize {
            let (snapshot, version) = self.read_snapshot(thread_id).await?;
            let (outcome, new_snapshot) = apply(snapshot).await?;
            let Some(new_snapshot) = new_snapshot else {
                return Ok(outcome);
            };
            if self.write_snapshot(&new_snapshot, version).await? {
                return Ok(outcome);
            }
        }
        Err(SessionThreadError::Backend(
            "thread state Postgres CAS retries exhausted".to_string(),
        ))
    }

    fn new_message_id() -> ThreadMessageId {
        ThreadMessageId::from_uuid(uuid::Uuid::new_v4())
    }
}

// ---------------------------------------------------------------------------
// SessionThreadService impl
// ---------------------------------------------------------------------------

#[async_trait]
impl SessionThreadService for PgSessionThreadService {
    async fn ensure_thread(
        &self,
        request: EnsureThreadRequest,
    ) -> Result<SessionThreadRecord, SessionThreadError> {
        let thread_id = match request.thread_id {
            Some(id) => id,
            None => ThreadId::new(uuid::Uuid::new_v4().to_string())
                .map_err(|e| SessionThreadError::GeneratedThreadId(e.to_string()))?,
        };
        if request.scope.tenant_id.as_str() != self.tenant_id.as_str() {
            return Err(SessionThreadError::ThreadScopeMismatch { thread_id });
        }
        // Check existing first (read-only fast path).
        let (snapshot, version) = self.read_snapshot(&thread_id).await?;
        if let Some(existing) = &snapshot.record {
            if existing.scope != request.scope {
                return Err(SessionThreadError::ThreadScopeMismatch {
                    thread_id: thread_id.clone(),
                });
            }
            return Ok(existing.clone());
        }
        let record = SessionThreadRecord {
            scope: request.scope,
            thread_id: thread_id.clone(),
            created_by_actor_id: request.created_by_actor_id,
            title: request.title,
            metadata_json: request.metadata_json,
            goal: None,
        };
        let new_snapshot = ThreadSnapshot {
            record: Some(record.clone()),
            ..ThreadSnapshot::default()
        };
        // Write; on conflict (another racing ensure) re-read and return existing.
        if !self.write_snapshot(&new_snapshot, version).await? {
            let (s2, _) = self.read_snapshot(&thread_id).await?;
            let existing = s2.record.ok_or_else(|| {
                SessionThreadError::Backend("thread vanished after race".to_string())
            })?;
            if existing.scope != record.scope {
                return Err(SessionThreadError::ThreadScopeMismatch { thread_id });
            }
            return Ok(existing);
        }
        // Derive title if absent using the (empty) message list — no messages yet on new thread.
        // Title derivation is a best-effort read-time operation; skip on creation.
        let _ = &new_snapshot; // snapshot already written above
        Ok(record)
    }

    async fn accept_inbound_message(
        &self,
        request: AcceptInboundMessageRequest,
    ) -> Result<AcceptedInboundMessage, SessionThreadError> {
        self.accept_inbound_transaction(request).await
    }

    async fn replay_accepted_inbound_message(
        &self,
        request: ReplayAcceptedInboundMessageRequest,
    ) -> Result<Option<AcceptedInboundMessageReplay>, SessionThreadError> {
        self.lookup_inbound_replay(request).await
    }

    async fn mark_message_submitted(
        &self,
        scope: &ThreadScope,
        thread_id: &ThreadId,
        message_id: ThreadMessageId,
        turn_id: String,
        turn_run_id: String,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = thread_id.clone();
        let scope = scope.clone();
        self.apply_optional(&thread_id, |mut snapshot| {
            let scope = scope.clone();
            let thread_id_inner = thread_id.clone();
            let turn_id = turn_id.clone();
            let turn_run_id = turn_run_id.clone();
            async move {
                check_thread_scope(&snapshot, &thread_id_inner, &scope)?;
                let message = snapshot
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == message_id)
                    .ok_or(SessionThreadError::Backend(format!(
                        "message {message_id:?} not found"
                    )))?;
                let changed =
                    crate::message_submission::bind_submission(message, &turn_id, &turn_run_id)?;
                let result = message.clone();
                Ok((result, changed.then_some(snapshot)))
            }
        })
        .await
    }

    async fn mark_message_deferred_busy(
        &self,
        scope: &ThreadScope,
        thread_id: &ThreadId,
        message_id: ThreadMessageId,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = thread_id.clone();
        let scope = scope.clone();
        self.apply(&thread_id, |mut snapshot| {
            let scope = scope.clone();
            let thread_id_inner = thread_id.clone();
            async move {
                check_thread_scope(&snapshot, &thread_id_inner, &scope)?;
                let message = snapshot
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == message_id)
                    .ok_or(SessionThreadError::Backend(format!(
                        "message {message_id:?} not found"
                    )))?;
                crate::message_submission::ensure_user_accepted(
                    message,
                    "mark_message_deferred_busy",
                )?;
                message.status = MessageStatus::DeferredBusy;
                message.turn_id = None;
                message.turn_run_id = None;
                let result = message.clone();
                Ok((result, snapshot))
            }
        })
        .await
    }

    async fn append_assistant_draft(
        &self,
        request: AppendAssistantDraftRequest,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = request.thread_id.clone();
        self.apply(&thread_id, |mut snapshot| {
            let request = request.clone();
            async move {
                check_thread_scope(&snapshot, &request.thread_id, &request.scope)?;
                // Idempotent: return existing finalized draft for same run if present.
                if let Some(existing) = snapshot.messages.iter().rev().find(|m| {
                    m.kind == MessageKind::Assistant
                        && m.turn_run_id.as_deref() == Some(request.turn_run_id.as_str())
                        && m.status == MessageStatus::Finalized
                }) {
                    return Ok((existing.clone(), snapshot));
                }
                let message_id = Self::new_message_id();
                let sequence = snapshot.next_sequence.checked_add(1).ok_or_else(|| {
                    SessionThreadError::Backend("thread message sequence exhausted".into())
                })?;
                snapshot.next_sequence = sequence;
                let message = ThreadMessageRecord {
                    message_id,
                    thread_id: request.thread_id,
                    sequence,
                    kind: MessageKind::Assistant,
                    status: MessageStatus::Draft,
                    actor_id: None,
                    source_binding_id: None,
                    reply_target_binding_id: None,
                    turn_id: None,
                    turn_run_id: Some(request.turn_run_id),
                    tool_result_ref: None,
                    tool_result_provider_call: None,
                    content: Some(request.content.into_text()),
                    redaction_ref: None,
                };
                snapshot.messages.push(message.clone());
                Ok((message, snapshot))
            }
        })
        .await
    }

    async fn append_tool_result_reference(
        &self,
        request: AppendToolResultReferenceRequest,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = request.thread_id.clone();
        self.apply(&thread_id, |mut snapshot| {
            let request = request.clone();
            async move {
                check_thread_scope(&snapshot, &request.thread_id, &request.scope)?;
                let envelope = ToolResultReferenceEnvelope::new_best_effort_model_observation(
                    request.result_ref.clone(),
                    request.safe_summary,
                    request.model_observation,
                )
                .map_err(SessionThreadError::Serialization)?;
                let content = serde_json::to_string(&envelope)
                    .map_err(|e| SessionThreadError::Serialization(e.to_string()))?;
                if let Some(provider_call) = &request.provider_call {
                    provider_call
                        .validate()
                        .map_err(SessionThreadError::Serialization)?;
                }
                if let Some(existing) = snapshot.messages.iter_mut().find(|message| {
                    message.kind == MessageKind::ToolResultReference
                        && message.status == MessageStatus::Finalized
                        && message.turn_run_id.as_deref() == Some(request.turn_run_id.as_str())
                        && message.tool_result_ref.as_deref() == Some(envelope.result_ref.as_str())
                }) {
                    if let Some(provider_call) = &request.provider_call {
                        match existing.tool_result_provider_call.as_ref() {
                            Some(previous) if previous == provider_call => {}
                            Some(_) => {
                                return Err(SessionThreadError::Serialization(
                                    "tool result provider metadata conflicts with existing record"
                                        .into(),
                                ));
                            }
                            None => {
                                existing.tool_result_provider_call = Some(provider_call.clone())
                            }
                        }
                    }
                    if let Some(observation) = envelope.model_observation.clone() {
                        let previous = existing.content.as_deref().ok_or_else(|| {
                            SessionThreadError::Serialization(
                                "tool result reference content is missing".into(),
                            )
                        })?;
                        if let Some(merged) =
                            ToolResultReferenceEnvelope::merge_model_observation_content_if_absent(
                                previous,
                                observation,
                            )
                            .map_err(SessionThreadError::Serialization)?
                        {
                            existing.content = Some(merged);
                        }
                    }
                    return Ok((existing.clone(), snapshot));
                }
                let message_id = Self::new_message_id();
                let sequence = snapshot.next_sequence.checked_add(1).ok_or_else(|| {
                    SessionThreadError::Backend("thread message sequence exhausted".into())
                })?;
                snapshot.next_sequence = sequence;
                let message = ThreadMessageRecord {
                    message_id,
                    thread_id: request.thread_id,
                    sequence,
                    kind: MessageKind::ToolResultReference,
                    status: MessageStatus::Finalized,
                    actor_id: None,
                    source_binding_id: None,
                    reply_target_binding_id: None,
                    turn_id: None,
                    turn_run_id: Some(request.turn_run_id),
                    tool_result_ref: Some(envelope.result_ref),
                    tool_result_provider_call: request.provider_call,
                    content: Some(content),
                    redaction_ref: None,
                };
                snapshot.messages.push(message.clone());
                Ok((message, snapshot))
            }
        })
        .await
    }

    async fn append_capability_display_preview(
        &self,
        request: AppendCapabilityDisplayPreviewRequest,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = request.thread_id.clone();
        self.apply(&thread_id, |mut snapshot| {
            let request = request.clone();
            async move {
                check_thread_scope(&snapshot, &request.thread_id, &request.scope)?;
                let message_id = Self::new_message_id();
                let sequence = snapshot.next_sequence.checked_add(1).ok_or_else(|| {
                    SessionThreadError::Backend("thread message sequence exhausted".into())
                })?;
                snapshot.next_sequence = sequence;
                let content_str = serde_json::to_string(&request.preview).unwrap_or_default();
                let message = ThreadMessageRecord {
                    message_id,
                    thread_id: request.thread_id,
                    sequence,
                    kind: MessageKind::CapabilityDisplayPreview,
                    status: MessageStatus::Finalized,
                    actor_id: None,
                    source_binding_id: None,
                    reply_target_binding_id: None,
                    turn_id: None,
                    turn_run_id: Some(request.turn_run_id),
                    tool_result_ref: None,
                    tool_result_provider_call: None,
                    content: Some(content_str),
                    redaction_ref: None,
                };
                snapshot.messages.push(message.clone());
                Ok((message, snapshot))
            }
        })
        .await
    }

    async fn update_tool_result_reference(
        &self,
        request: UpdateToolResultReferenceRequest,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = request.thread_id.clone();
        self.apply(&thread_id, |mut snapshot| {
            let request = request.clone();
            async move {
                check_thread_scope(&snapshot, &request.thread_id, &request.scope)?;
                let message = snapshot
                    .messages
                    .iter_mut()
                    .find(|m| {
                        m.kind == MessageKind::ToolResultReference
                            && m.status == MessageStatus::Finalized
                            && m.turn_run_id.as_deref() == Some(request.turn_run_id.as_str())
                            && m.tool_result_ref.as_deref() == Some(request.result_ref.as_str())
                    })
                    .ok_or_else(|| {
                        SessionThreadError::Backend(format!(
                            "tool result reference {} not found in thread {}",
                            request.result_ref, request.thread_id
                        ))
                    })?;
                let content = message.content.as_deref().ok_or_else(|| {
                    SessionThreadError::Serialization(
                        "tool result reference content is missing".to_string(),
                    )
                })?;
                let envelope = ToolResultReferenceEnvelope::from_json_str(content)
                    .map_err(SessionThreadError::Serialization)?
                    .with_safe_summary(request.safe_summary);
                message.content = Some(
                    serde_json::to_string(&envelope)
                        .map_err(|e| SessionThreadError::Serialization(e.to_string()))?,
                );
                let result = message.clone();
                Ok((result, snapshot))
            }
        })
        .await
    }

    async fn update_assistant_draft(
        &self,
        request: UpdateAssistantDraftRequest,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = request.thread_id.clone();
        self.apply(&thread_id, |mut snapshot| {
            let request = request.clone();
            async move {
                check_thread_scope(&snapshot, &request.thread_id, &request.scope)?;
                let message = snapshot
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == request.message_id)
                    .ok_or(SessionThreadError::Backend(format!(
                        "message {:?} not found",
                        request.message_id
                    )))?;
                message.content = Some(request.content.into_text());
                let result = message.clone();
                Ok((result, snapshot))
            }
        })
        .await
    }

    async fn finalize_assistant_message(
        &self,
        scope: &ThreadScope,
        thread_id: &ThreadId,
        message_id: ThreadMessageId,
        content: MessageContent,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = thread_id.clone();
        let scope = scope.clone();
        self.apply(&thread_id, |mut snapshot| {
            let scope = scope.clone();
            let thread_id_inner = thread_id.clone();
            let content = content.clone();
            async move {
                check_thread_scope(&snapshot, &thread_id_inner, &scope)?;
                let message = snapshot
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == message_id)
                    .ok_or(SessionThreadError::Backend(format!(
                        "message {message_id:?} not found"
                    )))?;
                message.status = MessageStatus::Finalized;
                message.content = Some(content.into_text());
                let result = message.clone();
                Ok((result, snapshot))
            }
        })
        .await
    }

    async fn redact_message(
        &self,
        request: RedactMessageRequest,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let thread_id = request.thread_id.clone();
        self.apply(&thread_id, |mut snapshot| {
            let request = request.clone();
            async move {
                check_thread_scope(&snapshot, &request.thread_id, &request.scope)?;
                let message = snapshot
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == request.message_id)
                    .ok_or(SessionThreadError::Backend(format!(
                        "message {:?} not found",
                        request.message_id
                    )))?;
                message.status = MessageStatus::Redacted;
                message.content = None;
                message.tool_result_provider_call = None;
                message.redaction_ref = Some(request.redaction_ref);
                let result = message.clone();
                Ok((result, snapshot))
            }
        })
        .await
    }

    async fn load_context_window(
        &self,
        request: LoadContextWindowRequest,
    ) -> Result<ContextWindow, SessionThreadError> {
        let (snapshot, _) = self.read_snapshot(&request.thread_id).await?;
        let thread_record = snapshot
            .record
            .as_ref()
            .ok_or(SessionThreadError::UnknownThread {
                thread_id: request.thread_id.clone(),
            })?;
        if thread_record.scope != request.scope {
            return Err(SessionThreadError::UnknownThread {
                thread_id: request.thread_id.clone(),
            });
        }
        let messages = context_messages_from_snapshot(
            &snapshot.messages,
            &snapshot.summary_artifacts,
            request.max_messages,
        );
        Ok(ContextWindow {
            thread_id: request.thread_id,
            messages,
        })
    }

    async fn load_context_messages(
        &self,
        request: LoadContextMessagesRequest,
    ) -> Result<ContextMessages, SessionThreadError> {
        let (snapshot, _) = self.read_snapshot(&request.thread_id).await?;
        let thread = snapshot
            .record
            .as_ref()
            .ok_or_else(|| SessionThreadError::UnknownThread {
                thread_id: request.thread_id.clone(),
            })?;
        if thread.scope != request.scope {
            return Err(SessionThreadError::UnknownThread {
                thread_id: request.thread_id,
            });
        }
        // Full eligible history may contain thousands of IDs. Index once,
        // preserving request order like the filesystem and in-memory stores.
        let by_id: std::collections::HashMap<_, _> = snapshot
            .messages
            .iter()
            .map(|message| (message.message_id, message))
            .collect();
        let messages = request
            .message_ids
            .iter()
            .filter_map(|id| by_id.get(id).copied())
            .filter_map(context_message_from_record)
            .collect();
        Ok(ContextMessages {
            thread_id: request.thread_id,
            messages,
        })
    }

    async fn list_thread_history(
        &self,
        request: ThreadHistoryRequest,
    ) -> Result<ThreadHistory, SessionThreadError> {
        let (snapshot, _) = self.read_snapshot(&request.thread_id).await?;
        let thread = snapshot
            .record
            .as_ref()
            .ok_or(SessionThreadError::UnknownThread {
                thread_id: request.thread_id.clone(),
            })?;
        if thread.scope != request.scope {
            return Err(SessionThreadError::UnknownThread {
                thread_id: request.thread_id,
            });
        }
        Ok(ThreadHistory {
            thread: thread.clone(),
            messages: snapshot
                .messages
                .iter()
                .map(|message| {
                    let mut visible = message.clone();
                    visible.tool_result_provider_call = None;
                    visible
                })
                .collect(),
            summary_artifacts: snapshot.summary_artifacts.clone(),
        })
    }

    async fn create_summary_artifact(
        &self,
        request: CreateSummaryArtifactRequest,
    ) -> Result<SummaryArtifact, SessionThreadError> {
        let thread_id = request.thread_id.clone();
        self.apply(&thread_id, |mut snapshot| {
            let request = request.clone();
            async move {
                check_thread_scope(&snapshot, &request.thread_id, &request.scope)?;
                let content = request.content.as_text().to_string();
                if let Some(existing) =
                    find_overlapping_summary(&snapshot.summary_artifacts, &request, &content)?
                {
                    return Ok((existing.clone(), snapshot));
                }
                let summary_id = crate::identifiers::SummaryArtifactId::new();
                let artifact = SummaryArtifact {
                    summary_id,
                    thread_id: request.thread_id,
                    start_sequence: request.start_sequence,
                    end_sequence: request.end_sequence,
                    summary_kind: request.summary_kind,
                    content,
                    model_context_policy: request.model_context_policy,
                };
                snapshot.summary_artifacts.push(artifact.clone());
                Ok((artifact, snapshot))
            }
        })
        .await
    }

    fn supports_resolve_scope(&self) -> bool {
        true
    }

    async fn resolve_scope(&self, thread_id: ThreadId) -> Result<ThreadScope, SessionThreadError> {
        let (snapshot, _) = self.read_snapshot(&thread_id).await?;
        snapshot
            .record
            .map(|r| r.scope)
            .ok_or(SessionThreadError::UnknownThread { thread_id })
    }

    async fn update_thread_goal(
        &self,
        request: UpdateThreadGoalRequest,
    ) -> Result<ThreadGoal, SessionThreadError> {
        let thread_id = request.thread_id.clone();
        self.apply(&thread_id, |mut snapshot| {
            let request = request.clone();
            async move {
                let record = snapshot
                    .record
                    .as_mut()
                    .ok_or(SessionThreadError::UnknownThread {
                        thread_id: request.thread_id,
                    })?;
                let goal = request.goal;
                record.goal = Some(goal.clone());
                Ok((goal, snapshot))
            }
        })
        .await
    }

    async fn read_thread_by_id(
        &self,
        thread_id: ThreadId,
    ) -> Result<SessionThreadRecord, SessionThreadError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT metadata FROM brassclaw_session_threads \
                 WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
                &[&thread_id.as_str(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        let row = row.ok_or(SessionThreadError::UnknownThread {
            thread_id: thread_id.clone(),
        })?;
        let payload: Value = row.get(0);
        let snapshot: ThreadSnapshot = serde_json::from_value(payload).map_err(map_json)?;
        snapshot
            .record
            .ok_or(SessionThreadError::UnknownThread { thread_id })
    }

    async fn close_thread(
        &self,
        scope: &ThreadScope,
        thread_id: &ThreadId,
    ) -> Result<(), SessionThreadError> {
        self.apply(thread_id, |mut snapshot| {
            let scope = scope.clone();
            let thread_id = thread_id.clone();
            async move {
                if snapshot
                    .record
                    .as_ref()
                    .is_none_or(|record| record.scope != scope)
                {
                    return Err(SessionThreadError::UnknownThread { thread_id });
                }
                snapshot.closed = true;
                Ok(((), snapshot))
            }
        })
        .await
    }

    async fn delete_thread(
        &self,
        scope: &ThreadScope,
        thread_id: &ThreadId,
    ) -> Result<(), SessionThreadError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let transaction = client.transaction().await.map_err(map_pg)?;
        self.locked_snapshot(&transaction, scope, thread_id)
            .await
            .map_err(|error| match error {
                SessionThreadError::ThreadScopeMismatch { .. } => {
                    SessionThreadError::UnknownThread {
                        thread_id: thread_id.clone(),
                    }
                }
                other => other,
            })?;
        transaction.execute(
            "DELETE FROM brassclaw_inbound_message_receipts WHERE tenant_id=$1 AND thread_id=$2",
            &[&self.tenant_id, &thread_id.as_str()],
        ).await.map_err(map_pg)?;
        let rows = transaction.execute(
            "UPDATE brassclaw_session_threads SET deleted_at=now() WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
            &[&self.tenant_id, &thread_id.as_str()],
        ).await.map_err(map_pg)?;
        if rows != 1 {
            return Err(SessionThreadError::UnknownThread {
                thread_id: thread_id.clone(),
            });
        }
        transaction.commit().await.map_err(map_pg)?;
        Ok(())
    }

    async fn list_threads_for_scope(
        &self,
        request: ListThreadsForScopeRequest,
    ) -> Result<ListThreadsForScopeResponse, SessionThreadError> {
        let user_id = request
            .scope
            .owner_user_id
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_else(|| brassclaw_host_api::SYSTEM_RESERVED_ID.to_string());
        let agent_id = request.scope.agent_id.to_string();
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT metadata FROM brassclaw_session_threads \
                 WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 \
                   AND deleted_at IS NULL \
                 ORDER BY updated_at DESC \
                 LIMIT 200",
                &[&self.tenant_id, &user_id, &agent_id],
            )
            .await
            .map_err(map_pg)?;
        let mut threads = Vec::new();
        for row in rows {
            let payload: Value = row.get(0);
            if let Ok(snapshot) = serde_json::from_value::<ThreadSnapshot>(payload)
                && let Some(record) = snapshot.record
            {
                threads.push(record);
            }
        }
        Ok(ListThreadsForScopeResponse {
            threads,
            next_cursor: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Snapshot projection helpers
// ---------------------------------------------------------------------------

fn check_thread_scope(
    snapshot: &ThreadSnapshot,
    thread_id: &ThreadId,
    scope: &ThreadScope,
) -> Result<(), SessionThreadError> {
    match snapshot.record.as_ref() {
        None => Err(SessionThreadError::UnknownThread {
            thread_id: thread_id.clone(),
        }),
        Some(r) if r.scope != *scope => Err(SessionThreadError::ThreadScopeMismatch {
            thread_id: thread_id.clone(),
        }),
        _ => Ok(()),
    }
}

fn is_model_context_visible(message: &ThreadMessageRecord) -> bool {
    matches!(
        message.kind,
        MessageKind::User
            | MessageKind::Assistant
            | MessageKind::ToolResultReference
            | MessageKind::CapabilityDisplayPreview
    ) && !matches!(
        message.status,
        MessageStatus::Redacted
            | MessageStatus::Deleted
            | MessageStatus::Draft
            | MessageStatus::Superseded
    ) && message.content.is_some()
}

fn context_messages_from_snapshot(
    messages: &[ThreadMessageRecord],
    summaries: &[SummaryArtifact],
    max_messages: usize,
) -> Vec<ContextMessage> {
    // Find the summary with ReplaceRangeWhenSelected policy that covers
    // the most recent compacted window, mirroring the in_memory impl.
    let replacement_summaries: Vec<&SummaryArtifact> = summaries
        .iter()
        .filter(|s| {
            s.model_context_policy == Some(SummaryModelContextPolicy::ReplaceRangeWhenSelected)
        })
        .collect();

    let mut skip_through: u64 = 0;
    let mut emitted_summary_ids = std::collections::HashSet::new();
    let mut result: Vec<ContextMessage> = Vec::new();

    for message in messages.iter().filter(|m| is_model_context_visible(m)) {
        if message.sequence <= skip_through {
            continue;
        }
        if let Some(summary) = replacement_summaries.iter().find(|s| {
            s.start_sequence <= message.sequence
                && message.sequence <= s.end_sequence
                && !emitted_summary_ids.contains(&s.summary_id)
        }) {
            result.push(ContextMessage {
                message_id: None,
                summary_id: Some(summary.summary_id),
                sequence: summary.start_sequence,
                kind: MessageKind::Summary,
                tool_result_provider_call: None,
                content: summary.content.clone(),
            });
            emitted_summary_ids.insert(summary.summary_id);
            skip_through = summary.end_sequence;
            continue;
        }
        if let Some(ctx) = context_message_from_record(message) {
            result.push(ctx);
        }
    }

    // Take the last `max_messages` messages.
    if result.len() > max_messages {
        let drain_count = result.len() - max_messages;
        result.drain(0..drain_count);
    }
    result
}

fn context_message_from_record(message: &ThreadMessageRecord) -> Option<ContextMessage> {
    if !is_model_context_visible(message) {
        return None;
    }
    Some(ContextMessage {
        message_id: Some(message.message_id),
        summary_id: None,
        sequence: message.sequence,
        kind: message.kind,
        tool_result_provider_call: message.tool_result_provider_call.clone(),
        content: message.content.clone()?,
    })
}
