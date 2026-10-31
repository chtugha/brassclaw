//! Internal stored-scope rehydration; never public ingress or an authority grant.
use brassclaw_host_api::ResourceScope;
use serde::Deserialize;

// Only internal owners may use this decoder, with their existing ciphertext or
// scoped-storage checks and full caller-owner validation before effects. Parsing
// alone grants nothing. Public TenantId/UserId parsing still rejects the sentinel.
pub(crate) fn deserialize_stored_scope<'de, D>(deserializer: D) -> Result<ResourceScope, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct StoredScope {
        tenant_id: String,
        user_id: String,
        agent_id: Option<brassclaw_host_api::AgentId>,
        project_id: Option<brassclaw_host_api::ProjectId>,
        thread_id: Option<brassclaw_host_api::ThreadId>,
        invocation_id: brassclaw_host_api::InvocationId,
    }
    let stored = StoredScope::deserialize(deserializer)?;
    if stored.tenant_id == brassclaw_host_api::SYSTEM_RESERVED_ID
        && stored.user_id == brassclaw_host_api::SYSTEM_RESERVED_ID
        && stored.agent_id.is_none()
        && stored.project_id.is_none()
        && stored.thread_id.is_none()
    {
        return Ok(ResourceScope {
            invocation_id: stored.invocation_id,
            ..ResourceScope::system()
        });
    }
    Ok(ResourceScope {
        tenant_id: brassclaw_host_api::TenantId::new(stored.tenant_id)
            .map_err(serde::de::Error::custom)?,
        user_id: brassclaw_host_api::UserId::new(stored.user_id)
            .map_err(serde::de::Error::custom)?,
        agent_id: stored.agent_id,
        project_id: stored.project_id,
        thread_id: stored.thread_id,
        invocation_id: stored.invocation_id,
    })
}
