//! Canonical event receipt and actual user message commit in one transaction.
use super::*;

impl PgSessionThreadService {
    pub(super) async fn accept_inbound_transaction(
        &self,
        request: AcceptInboundMessageRequest,
    ) -> Result<AcceptedInboundMessage, SessionThreadError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let transaction = client.transaction().await.map_err(map_pg)?;
        match self.accept_inbound_locked(&transaction, &request).await {
            Ok(accepted) => {
                if accepted.idempotent_replay {
                    transaction.rollback().await.map_err(map_pg)?;
                } else {
                    transaction.commit().await.map_err(map_pg)?;
                }
                Ok(accepted)
            }
            Err(error) => {
                transaction.rollback().await.map_err(map_pg)?;
                Err(error)
            }
        }
    }

    async fn accept_inbound_locked(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        request: &AcceptInboundMessageRequest,
    ) -> Result<AcceptedInboundMessage, SessionThreadError> {
        let (mut snapshot, version) = self
            .locked_snapshot(transaction, &request.scope, &request.thread_id)
            .await?;
        let message_id = Self::new_message_id();
        if let (Some(source), Some(event)) =
            (&request.source_binding_id, &request.external_event_id)
        {
            let project = request.scope.project_id.as_ref().map(|id| id.as_str());
            let owner = request.scope.owner_user_id.as_ref().map(|id| id.as_str());
            // PostgreSQL canonicalizes the typed tuple identically to V176.
            // The raw fields are checked too; a digest collision cannot bind
            // another event. The PK serializes contenders across threads.
            let created = transaction.execute(
                "INSERT INTO brassclaw_inbound_message_receipts \
                 (tenant_id,receipt_key,agent_id,project_id,owner_user_id,source_binding_id,external_event_id,thread_id,message_id) \
                 VALUES ($1,sha256(convert_to(jsonb_build_array($1::text,$3::text,$4::text,$5::text,$6::text,$7::text)::text,'UTF8')),$3,$4,$5,$6,$7,$2,$8) \
                 ON CONFLICT (tenant_id,receipt_key) DO NOTHING",
                &[&self.tenant_id,&request.thread_id.as_str(),&request.scope.agent_id.as_str(),
                  &project,&owner,source,event,&message_id.as_uuid()],
            ).await.map_err(map_pg)?;
            let mut old_entries = snapshot.inbound_idempotency.iter().filter(|entry| {
                entry.source_binding_id == *source && entry.external_event_id == *event
            });
            if created == 0 {
                let winner = transaction.query_one(
                    "SELECT thread_id,message_id,agent_id,project_id,owner_user_id,source_binding_id,external_event_id \
                     FROM brassclaw_inbound_message_receipts WHERE tenant_id=$1 AND receipt_key= \
                     sha256(convert_to(jsonb_build_array($1::text,$2::text,$3::text,$4::text,$5::text,$6::text)::text,'UTF8'))",
                    &[&self.tenant_id,&request.scope.agent_id.as_str(),&project,&owner,source,event],
                ).await.map_err(map_pg)?;
                if winner.get::<_, &str>(2) != request.scope.agent_id.as_str()
                    || winner.get::<_, Option<&str>>(3) != project
                    || winner.get::<_, Option<&str>>(4) != owner
                    || winner.get::<_, &str>(5) != source.as_str()
                    || winner.get::<_, &str>(6) != event.as_str()
                {
                    return Err(SessionThreadError::Backend(
                        "inbound receipt identity mismatch".into(),
                    ));
                }
                let original_thread = ThreadId::new(winner.get::<_, String>(0)).map_err(|_| {
                    SessionThreadError::Backend("invalid inbound receipt thread".into())
                })?;
                if original_thread != request.thread_id {
                    return Err(SessionThreadError::IdempotentReplayThreadMismatch {
                        stored_thread_id: original_thread,
                        requested_thread_id: request.thread_id.clone(),
                    });
                }
                let original_id = ThreadMessageId::from_uuid(winner.get::<_, uuid::Uuid>(1));
                let invalid =
                    || SessionThreadError::Backend("inbound receipt identity mismatch".into());
                let entry = old_entries.next().ok_or_else(invalid)?;
                if old_entries.next().is_some()
                    || entry.thread_id != request.thread_id.as_str()
                    || ThreadMessageId::parse(&entry.message_id).map_err(|_| invalid())?
                        != original_id
                    || entry.scope_tenant_id != request.scope.tenant_id.as_str()
                    || entry.scope_agent_id != request.scope.agent_id.as_str()
                    || entry.scope_owner_user_id.as_deref() != owner
                {
                    return Err(invalid());
                }
                let mut messages = snapshot
                    .messages
                    .iter()
                    .filter(|message| message.message_id == original_id);
                let message = messages.next().ok_or_else(invalid)?;
                if messages.next().is_some() {
                    return Err(invalid());
                }
                return crate::inbound_idempotency::acknowledged_input(message, request);
            }
            if old_entries.next().is_some() {
                // A legacy receipt missing from the canonical backfill is
                // corruption; never append another event under its key.
                return Err(SessionThreadError::Backend(
                    "canonical inbound receipt is missing".into(),
                ));
            }
        }
        if snapshot.closed {
            return Err(SessionThreadError::Backend("thread is closed".into()));
        }
        let sequence = snapshot.next_sequence.checked_add(1).ok_or_else(|| {
            SessionThreadError::Backend("thread message sequence exhausted".into())
        })?;
        snapshot.next_sequence = sequence;
        let message = ThreadMessageRecord {
            message_id,
            thread_id: request.thread_id.clone(),
            sequence,
            kind: MessageKind::User,
            status: MessageStatus::Accepted,
            actor_id: Some(request.actor_id.clone()),
            source_binding_id: request.source_binding_id.clone(),
            reply_target_binding_id: request.reply_target_binding_id.clone(),
            turn_id: None,
            turn_run_id: None,
            tool_result_ref: None,
            tool_result_provider_call: None,
            content: Some(request.content.as_text().to_owned()),
            redaction_ref: None,
        };
        if let (Some(source), Some(event)) =
            (&request.source_binding_id, &request.external_event_id)
        {
            snapshot.inbound_idempotency.push(InboundIdempotencyEntry {
                scope_tenant_id: request.scope.tenant_id.to_string(),
                scope_agent_id: request.scope.agent_id.to_string(),
                scope_owner_user_id: request
                    .scope
                    .owner_user_id
                    .as_ref()
                    .map(|id| id.to_string()),
                source_binding_id: source.clone(),
                external_event_id: event.clone(),
                thread_id: request.thread_id.to_string(),
                message_id: message_id.to_string(),
            });
        }
        snapshot.messages.push(message);
        self.write_locked_snapshot(transaction, &snapshot, version)
            .await?;
        Ok(AcceptedInboundMessage {
            thread_id: request.thread_id.clone(),
            message_id,
            sequence,
            idempotent_replay: false,
        })
    }
}
