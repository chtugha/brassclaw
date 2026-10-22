//! Transcript metadata participation in the caller's admission transaction.
use super::*;
use crate::SubmittedUserMessageRequest;

impl PgSessionThreadService {
    /// Bind the coordinator's exact references while its admission transaction
    /// is still open. The caller commits both or neither. This performs no
    /// admission/lifecycle decision, external effect or separate pool checkout.
    ///
    /// The thread row is locked after the caller's turn-snapshot row. Callers
    /// must use that lock order and must not hold a transcript lock while
    /// starting a coordinator transaction. An exact repeated binding is read
    /// only; another binding or an invalid/redacted input aborts admission.
    pub async fn bind_submission_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        request: SubmittedUserMessageRequest,
    ) -> Result<ThreadMessageRecord, SessionThreadError> {
        let (mut snapshot, version) = self
            .locked_snapshot(transaction, &request.scope, &request.thread_id)
            .await?;
        let message = snapshot
            .messages
            .iter_mut()
            .find(|message| message.message_id == request.message_id)
            .ok_or(SessionThreadError::SubmittedInputMismatch {
                message_id: request.message_id,
            })?;
        if message.thread_id != request.thread_id {
            return Err(SessionThreadError::SubmittedInputMismatch {
                message_id: request.message_id,
            });
        }
        let changed = crate::message_submission::bind_submission(
            message,
            &request.turn_id,
            &request.turn_run_id,
        )?;
        let result = message.clone();
        if !changed {
            return Ok(result);
        }
        self.write_locked_snapshot(transaction, &snapshot, version)
            .await?;
        Ok(result)
    }
    pub(super) async fn locked_snapshot(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ThreadScope,
        thread_id: &ThreadId,
    ) -> Result<(ThreadSnapshot, i64), SessionThreadError> {
        if scope.tenant_id.as_str() != self.tenant_id.as_str() {
            return Err(SessionThreadError::ThreadScopeMismatch {
                thread_id: thread_id.clone(),
            });
        }
        let row = transaction
            .query_opt(
                "SELECT metadata,version,user_id,agent_id,project_id \
                 FROM brassclaw_session_threads \
                 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
                &[&self.tenant_id, &thread_id.as_str()],
            )
            .await
            .map_err(map_pg)?
            .ok_or_else(|| SessionThreadError::UnknownThread {
                thread_id: thread_id.clone(),
            })?;
        let version: i64 = row.get(1);
        let snapshot = self.decode_snapshot(thread_id, row.get(0), version)?;
        check_thread_scope(&snapshot, thread_id, scope)?;
        let expected_user = scope
            .owner_user_id
            .as_ref()
            .map(|user| user.as_str())
            .unwrap_or(brassclaw_host_api::SYSTEM_RESERVED_ID);
        if row.get::<_, &str>(2) != expected_user
            || row.get::<_, &str>(3) != scope.agent_id.as_str()
            || row.get::<_, Option<&str>>(4)
                != scope.project_id.as_ref().map(|project| project.as_str())
        {
            return Err(SessionThreadError::ThreadScopeMismatch {
                thread_id: thread_id.clone(),
            });
        }
        Ok((snapshot, version))
    }

    pub(super) async fn write_locked_snapshot(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        snapshot: &ThreadSnapshot,
        version: i64,
    ) -> Result<(), SessionThreadError> {
        let thread_id = &snapshot
            .record
            .as_ref()
            .ok_or_else(|| SessionThreadError::Backend("snapshot has no record".into()))?
            .thread_id;
        let next_version = version.checked_add(1).ok_or_else(|| {
            SessionThreadError::Backend("thread snapshot version exhausted".into())
        })?;
        let metadata = serde_json::to_value(snapshot).map_err(map_json)?;
        let rows = transaction
            .execute(
                "UPDATE brassclaw_session_threads SET metadata=$3,version=$4,updated_at=now() \
                 WHERE tenant_id=$1 AND id=$2 AND version=$5 AND deleted_at IS NULL",
                &[
                    &self.tenant_id,
                    &thread_id.as_str(),
                    &metadata,
                    &next_version,
                    &version,
                ],
            )
            .await
            .map_err(map_pg)?;
        if rows != 1 {
            return Err(SessionThreadError::Backend(
                "thread admission binding changed while locked".into(),
            ));
        }
        Ok(())
    }
}
