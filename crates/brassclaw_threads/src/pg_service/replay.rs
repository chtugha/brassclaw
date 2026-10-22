//! Exact existing inbound receipts; read-only metadata, never new admission.
use super::*;

impl PgSessionThreadService {
    pub(super) async fn lookup_inbound_replay(
        &self,
        request: ReplayAcceptedInboundMessageRequest,
    ) -> Result<Option<AcceptedInboundMessageReplay>, SessionThreadError> {
        if request.scope.tenant_id.as_str() != self.tenant_id.as_str() {
            return Ok(None);
        }
        let user = request
            .scope
            .owner_user_id
            .as_ref()
            .map(|id| id.as_str())
            .unwrap_or(brassclaw_host_api::SYSTEM_RESERVED_ID);
        let project = request.scope.project_id.as_ref().map(|id| id.as_str());
        let needle = serde_json::json!([{
            "source_binding_id":request.source_binding_id.as_str(),
            "external_event_id":request.external_event_id.as_str(),
        }]);
        let client = self.pool.get().await.map_err(map_pool)?;
        // Select only candidate receipts in the requested canonical scope.
        // Unrelated transcripts are neither transferred nor deserialized.
        let rows = client
            .query(
                "SELECT id,metadata,version FROM brassclaw_session_threads \
             WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 \
               AND project_id IS NOT DISTINCT FROM $4 AND deleted_at IS NULL \
               AND (metadata->'inbound_idempotency') @> $5::jsonb",
                &[
                    &self.tenant_id,
                    &user,
                    &request.scope.agent_id.as_str(),
                    &project,
                    &needle,
                ],
            )
            .await
            .map_err(map_pg)?;
        if rows.len() > 1 {
            return Err(SessionThreadError::Backend(
                "inbound receipt is ambiguous".into(),
            ));
        }
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let thread_id = ThreadId::new(row.get::<_, String>(0))
            .map_err(|_| SessionThreadError::Backend("invalid inbound receipt thread".into()))?;
        let snapshot = self.decode_snapshot(&thread_id, row.get(1), row.get(2))?;
        check_thread_scope(&snapshot, &thread_id, &request.scope)?;
        let mut entries = snapshot.inbound_idempotency.iter().filter(|entry| {
            entry.source_binding_id == request.source_binding_id
                && entry.external_event_id == request.external_event_id
        });
        let invalid = || SessionThreadError::Backend("inbound receipt identity mismatch".into());
        let entry = entries.next().ok_or_else(invalid)?;
        if entries.next().is_some() {
            return Err(SessionThreadError::Backend(
                "inbound receipt is ambiguous".into(),
            ));
        }
        if entry.scope_tenant_id != request.scope.tenant_id.as_str()
            || entry.scope_agent_id != request.scope.agent_id.as_str()
            || entry.scope_owner_user_id.as_deref()
                != request.scope.owner_user_id.as_ref().map(|id| id.as_str())
            || entry.thread_id != thread_id.as_str()
        {
            return Err(invalid());
        }
        let message_id = ThreadMessageId::parse(&entry.message_id).map_err(|_| invalid())?;
        let mut messages = snapshot
            .messages
            .iter()
            .filter(|message| message.message_id == message_id);
        let message = messages.next().ok_or_else(invalid)?;
        if messages.next().is_some()
            || message.thread_id != thread_id
            || message.kind != MessageKind::User
            || message.source_binding_id.as_deref() != Some(request.source_binding_id.as_str())
        {
            return Err(invalid());
        }
        if message.actor_id.as_deref() != Some(request.actor_id.as_str()) {
            return Ok(None);
        }
        Ok(Some(AcceptedInboundMessageReplay {
            scope: request.scope,
            thread_id,
            message_id,
            sequence: message.sequence,
            status: message.status,
            actor_id: message.actor_id.clone(),
            source_binding_id: message.source_binding_id.clone(),
            reply_target_binding_id: message.reply_target_binding_id.clone(),
            turn_run_id: message.turn_run_id.clone(),
        }))
    }
}
