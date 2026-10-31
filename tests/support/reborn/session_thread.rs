#![allow(unreachable_pub)] // Integration test support — items pub for use across test modules.
use std::sync::Arc;

use brassclaw_host_api::ThreadId;
use brassclaw_pg::PgPool;
use brassclaw_threads::{
    PgSessionThreadService, SessionThreadService, ThreadHistoryRequest, ThreadMessageRecord,
    ThreadScope,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RebornThreadHarnessError {
    #[error("thread service failed: {0}")]
    Thread(#[from] brassclaw_threads::SessionThreadError),
    #[error("thread history does not contain final assistant reply containing {0:?}")]
    MissingFinalReply(String),
}

pub struct RebornThreadHarness {
    pub scope: ThreadScope,
    pub service: Arc<PgSessionThreadService>,
    pool: Arc<PgPool>,
}

impl RebornThreadHarness {
    pub fn postgres(scope: ThreadScope, pool: Arc<PgPool>) -> Self {
        let service = Arc::new(PgSessionThreadService::new(
            Arc::clone(&pool),
            scope.tenant_id.as_str(),
        ));
        Self {
            scope,
            service,
            pool,
        }
    }

    pub fn service_instance(&self) -> PgSessionThreadService {
        PgSessionThreadService::new(Arc::clone(&self.pool), self.scope.tenant_id.as_str())
    }

    pub async fn history(
        &self,
        thread_id: ThreadId,
    ) -> Result<Vec<ThreadMessageRecord>, RebornThreadHarnessError> {
        Ok(self
            .service
            .list_thread_history(ThreadHistoryRequest {
                scope: self.scope.clone(),
                thread_id,
            })
            .await?
            .messages)
    }

    pub async fn assert_final_reply(
        &self,
        thread_id: ThreadId,
        text: &str,
    ) -> Result<(), RebornThreadHarnessError> {
        let history = self.history(thread_id).await?;
        let found = history
            .iter()
            .rev()
            .find(|message| {
                message.kind == brassclaw_threads::MessageKind::Assistant
                    && message.status == brassclaw_threads::MessageStatus::Finalized
            })
            .is_some_and(|message| {
                message
                    .content
                    .as_deref()
                    .is_some_and(|content| content.contains(text))
            });
        if found {
            Ok(())
        } else {
            Err(RebornThreadHarnessError::MissingFinalReply(
                text.to_string(),
            ))
        }
    }
}
