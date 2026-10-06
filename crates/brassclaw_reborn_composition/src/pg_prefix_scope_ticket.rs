//! Persistence for short-lived, one-use prefix-regeneration scope tickets.

use std::sync::Arc;

use brassclaw_pg::PgPool;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct PrefixScopeTicket {
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
    pub conversation_id: String,
    pub request_id: Uuid,
}

#[derive(Clone)]
pub(crate) struct PgPrefixScopeTicketStore {
    pool: Arc<PgPool>,
}

impl PgPrefixScopeTicketStore {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub(crate) async fn issue(
        &self,
        raw_ticket: &str,
        ticket: &PrefixScopeTicket,
    ) -> Result<(), String> {
        let digest = ticket_digest(raw_ticket);
        let client = self.pool.get().await.map_err(|e| e.to_string())?;
        client
            .execute(
                "INSERT INTO reborn_prefix_scope_tickets
                 (ticket_hash, tenant_id, user_id, agent_id, project_id,
                  conversation_id, request_id, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,now() + interval '5 minutes')",
                &[
                    &digest,
                    &ticket.tenant_id,
                    &ticket.user_id,
                    &ticket.agent_id,
                    &ticket.project_id,
                    &ticket.conversation_id,
                    &ticket.request_id,
                ],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub(crate) async fn resolve(&self, raw_ticket: &str) -> Result<PrefixScopeTicket, String> {
        let digest = ticket_digest(raw_ticket);
        let client = self.pool.get().await.map_err(|e| e.to_string())?;
        let row = client
            .query_opt(
                "SELECT tenant_id, user_id, agent_id, project_id, conversation_id, request_id
             FROM reborn_prefix_scope_tickets
             WHERE ticket_hash = $1 AND consumed_at IS NULL AND expires_at > now()",
                &[&digest],
            )
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "scope ticket is invalid, expired, or consumed".to_string())?;
        Ok(PrefixScopeTicket {
            tenant_id: row.get(0),
            user_id: row.get(1),
            agent_id: row.get(2),
            project_id: row.get(3),
            conversation_id: row.get(4),
            request_id: row.get(5),
        })
    }

    pub(crate) async fn acquire_lease(&self, ticket: &PrefixScopeTicket) -> Result<Uuid, String> {
        let token = Uuid::new_v4();
        let client = self.pool.get().await.map_err(|e| e.to_string())?;
        let row = client
            .query_opt(
                "INSERT INTO reborn_prefix_scope_leases
                 (tenant_id,user_id,agent_id,project_id,lease_token,expires_at)
             VALUES ($1,$2,$3,$4,$5,now()+interval '15 minutes')
             ON CONFLICT (tenant_id,user_id,agent_id,project_id)
             DO UPDATE SET lease_token=EXCLUDED.lease_token,
                           expires_at=EXCLUDED.expires_at
             WHERE reborn_prefix_scope_leases.expires_at <= now()
             RETURNING lease_token",
                &[
                    &ticket.tenant_id,
                    &ticket.user_id,
                    &ticket.agent_id,
                    &ticket.project_id,
                    &token,
                ],
            )
            .await
            .map_err(|e| e.to_string())?;
        row.ok_or_else(|| "prefix regeneration is already active for this scope".to_string())?;
        Ok(token)
    }

    pub(crate) async fn release_lease(
        &self,
        ticket: &PrefixScopeTicket,
        token: Uuid,
    ) -> Result<(), String> {
        let client = self.pool.get().await.map_err(|e| e.to_string())?;
        client.execute(
            "DELETE FROM reborn_prefix_scope_leases
             WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND lease_token=$5",
            &[&ticket.tenant_id,&ticket.user_id,&ticket.agent_id,&ticket.project_id,&token],
        ).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub(crate) fn ticket_digest(raw_ticket: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(raw_ticket.as_bytes());
    hex::encode(digest.finalize())
}
