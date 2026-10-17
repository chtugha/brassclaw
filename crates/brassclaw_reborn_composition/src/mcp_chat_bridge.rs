//! MCP transport owns correlation only. All execution uses ordinary chat.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};

use brassclaw_turns::run_profile::CapabilitySurfaceVersion;
use deadpool_postgres::Pool;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::mcp_recipe_catalogue::{McpRecipeDiscovery, McpRecipeDiscoverySnapshot};
use crate::runtime::RebornRuntime;

#[cfg(test)]
#[path = "mcp_transport_limits_tests.rs"]
mod transport_limits_tests;
#[cfg(test)]
pub(crate) use transport_limits_tests::assert_native_transport_limits;

#[derive(Debug, thiserror::Error)]
pub enum McpChatError {
    #[error("MCP exchange is unavailable or disconnected")]
    Unavailable,
    #[error("MCP command advertisement is stale or invalid")]
    InvalidCommand,
    #[error("MCP request ID conflicts with its original command")]
    RequestConflict,
    #[error("MCP original chat outcome is unresolved; no replacement was submitted")]
    Unresolved,
    #[error("MCP original chat references are inconsistent; no replacement was submitted")]
    InconsistentReferences,
    #[error("MCP durable correlation storage failed")]
    Storage,
}

impl From<crate::runtime::CorrelatedChatReferenceError> for McpChatError {
    fn from(error: crate::runtime::CorrelatedChatReferenceError) -> Self {
        match error {
            crate::runtime::CorrelatedChatReferenceError::Missing => Self::Unresolved,
            crate::runtime::CorrelatedChatReferenceError::Inconsistent => {
                Self::InconsistentReferences
            }
            crate::runtime::CorrelatedChatReferenceError::Storage(_) => Self::Storage,
        }
    }
}

struct CachedReceipt<'a> {
    response: &'a Value,
    user_ref: Option<&'a str>,
    reply_ref: Option<&'a str>,
}

struct Exchange {
    id: Uuid,
    parent_run_id: Uuid,
    parent_surface_version: Option<CapabilitySurfaceVersion>,
    protocol_state: AtomicU8,
    advertised: Arc<McpRecipeDiscoverySnapshot>,
    cancellation: CancellationToken,
    calls: Mutex<HashMap<String, RegisteredCall>>,
}

struct InFlightCall {
    cancellation: CancellationToken,
}

struct RegisteredCall {
    call: Arc<InFlightCall>,
    waiters: usize,
}

/// Only HTTP waiters own leases. Token clones held by admitted work do not
/// extend this index; durable correlation owns recovery and deduplication.
struct CallLease {
    exchange: Arc<Exchange>,
    key: String,
    call: Arc<InFlightCall>,
}

/// Match PostgreSQL JSONB scalar equality used by the durable reservation.
/// Decimal spelling must not split one request's waiter/cancellation owner.
/// Normalize the serialized decimal exactly; converting through f64 would
/// merge distinct large integer IDs. Strings keep their separate identity.
fn request_key(id: &Value) -> Result<String, McpChatError> {
    let number = match id {
        Value::String(value) => return Ok(format!("string:{value}")),
        Value::Number(number) => number.to_string(),
        _ => return Err(McpChatError::RequestConflict),
    };
    let (mantissa, exponent) = number
        .split_once('e')
        .or_else(|| number.split_once('E'))
        .unwrap_or((&number, "0"));
    let exponent: i64 = exponent
        .parse()
        .map_err(|_| McpChatError::RequestConflict)?;
    let (negative, unsigned) = match mantissa.strip_prefix('-') {
        Some(unsigned) => ("-", unsigned),
        None => ("", mantissa),
    };
    let fractional = unsigned.split_once('.').map_or(0, |(_, tail)| tail.len());
    let digits = unsigned.replace('.', "");
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Ok("number:0".into());
    }
    let coefficient = significant.trim_end_matches('0');
    let trailing = significant.len() - coefficient.len();
    let exponent = exponent
        .checked_sub(i64::try_from(fractional).map_err(|_| McpChatError::RequestConflict)?)
        .and_then(|value| value.checked_add(i64::try_from(trailing).ok()?))
        .ok_or(McpChatError::RequestConflict)?;
    Ok(format!("number:{negative}{coefficient}e{exponent}"))
}

impl Exchange {
    fn acquire_call(
        self: &Arc<Self>,
        key: String,
        capacity: u32,
    ) -> Result<CallLease, McpChatError> {
        let mut calls = self.calls.lock().map_err(|_| McpChatError::Unavailable)?;
        // This bounds simultaneous identities, never completed request history.
        if capacity == 0 || (calls.len() >= capacity as usize && !calls.contains_key(&key)) {
            return Err(McpChatError::Unavailable);
        }
        let registered = calls.entry(key.clone()).or_insert_with(|| RegisteredCall {
            call: Arc::new(InFlightCall {
                cancellation: self.cancellation.child_token(),
            }),
            waiters: 0,
        });
        registered.waiters = registered
            .waiters
            .checked_add(1)
            .ok_or(McpChatError::Unavailable)?;
        Ok(CallLease {
            exchange: self.clone(),
            key,
            call: registered.call.clone(),
        })
    }
}

impl Drop for CallLease {
    fn drop(&mut self) {
        let Ok(mut calls) = self.exchange.calls.lock() else {
            // Corrupted ownership must fence the exchange, never leave a live
            // credential admitting calls whose cancellation cannot be tracked.
            self.exchange.cancellation.cancel();
            return;
        };
        let Some(registered) = calls.get_mut(&self.key) else {
            return;
        };
        if !Arc::ptr_eq(&registered.call, &self.call) {
            return;
        }
        let Some(remaining) = registered.waiters.checked_sub(1) else {
            self.exchange.cancellation.cancel();
            return;
        };
        registered.waiters = remaining;
        if remaining == 0 {
            calls.remove(&self.key);
            self.call.cancellation.cancel();
            if calls.is_empty() {
                // Reclaim the completed batch as a whole, rather than retain
                // a large peak-sized index for the provider exchange lifetime.
                *calls = HashMap::new();
            }
        }
    }
}

/// Product-owned transport facade. No component selection or Tool dispatch API.
pub struct McpChatBridge {
    runtime: Weak<RebornRuntime>,
    pool: Arc<Pool>,
    discovery: Arc<McpRecipeDiscovery>,
    owner_scope: Value,
    startup_prepared: tokio::sync::Mutex<bool>,
    exchanges: Mutex<HashMap<String, Arc<Exchange>>>,
}

/// Kohai owns this lease across intermediate model Tool requests. Its Drop
/// immediately revokes the credential even if durable cleanup is interrupted.
/// Never put the bearer credential into a prompt, transcript or diagnostic.
pub struct McpProviderExchange {
    bridge: Arc<McpChatBridge>,
    exchange: Arc<Exchange>,
    token: String,
    token_hash: String,
}
impl McpProviderExchange {
    pub(crate) async fn transport_limits(
        &self,
    ) -> Result<brassclaw_host_api::MontyExecutionLimits, McpChatError> {
        self.bridge.transport_limits(&self.token).await
    }

    pub fn id(&self) -> Uuid {
        self.exchange.id
    }
    pub fn bearer_token(&self) -> &str {
        &self.token
    }
    pub fn tools_list(&self) -> Value {
        self.exchange.advertised.tools_list()
    }
    pub(crate) fn matches_advertisement(&self, snapshot: &McpRecipeDiscoverySnapshot) -> bool {
        self.exchange.advertised.same_contracts(snapshot)
    }
    pub async fn disconnect(&self) -> Result<(), McpChatError> {
        self.revoke();
        self.bridge.pool.get().await.map_err(|_| McpChatError::Storage)?
            .execute("UPDATE brassclaw_mcp_exchanges SET active=false, disconnected_at=COALESCE(disconnected_at,clock_timestamp()) WHERE exchange_id=$1", &[&self.exchange.id])
            .await.map_err(|_| McpChatError::Storage)?;
        Ok(())
    }
    pub(crate) fn revoke(&self) {
        self.exchange.cancellation.cancel();
        if let Ok(mut exchanges) = self.bridge.exchanges.lock() {
            exchanges.remove(&self.token_hash);
        }
    }
}
impl Drop for McpProviderExchange {
    fn drop(&mut self) {
        self.revoke();
    }
}

impl McpChatBridge {
    /// Authenticate before potentially expensive body reads, and recheck after
    /// awaiting the acknowledged settings view in case the exchange was fenced.
    pub(crate) async fn transport_limits(
        &self,
        token: &str,
    ) -> Result<brassclaw_host_api::MontyExecutionLimits, McpChatError> {
        self.authenticate(token)?;
        let runtime = self.runtime.upgrade().ok_or(McpChatError::Unavailable)?;
        let limits = runtime
            .acknowledged_monty_execution_limits()
            .await
            .map_err(|_| McpChatError::Unavailable)?;
        self.authenticate(token)?;
        Ok(limits)
    }

    async fn acquire_call(
        &self,
        exchange: &Arc<Exchange>,
        key: String,
    ) -> Result<CallLease, McpChatError> {
        let runtime = self.runtime.upgrade().ok_or(McpChatError::Unavailable)?;
        let limits = runtime
            .acknowledged_monty_execution_limits()
            .await
            .map_err(|_| McpChatError::Unavailable)?;
        exchange.acquire_call(key, limits.max_mcp_inflight_calls)
    }

    pub(crate) fn new(
        runtime: Weak<RebornRuntime>,
        pool: Arc<Pool>,
        discovery: Arc<McpRecipeDiscovery>,
        owner_scope: Value,
    ) -> Arc<Self> {
        Arc::new(Self {
            runtime,
            pool,
            discovery,
            owner_scope,
            startup_prepared: tokio::sync::Mutex::new(false),
            exchanges: Mutex::new(HashMap::new()),
        })
    }

    /// Call after final prefix composition, immediately before provider send.
    /// Capturing an advertisement neither selects a Recipe nor grants a Tool.
    pub async fn connect(
        self: &Arc<Self>,
        parent_run_id: Uuid,
        surface: CapabilitySurfaceVersion,
    ) -> Result<McpProviderExchange, McpChatError> {
        let runtime = self.runtime.upgrade().ok_or(McpChatError::Unavailable)?;
        let advertised = runtime
            .mcp_surface_snapshot(parent_run_id, &surface)
            .map_err(|_| McpChatError::Unavailable)?
            .ok_or(McpChatError::Unavailable)?;
        let exchange = self
            .connect_selected(parent_run_id, Some(surface.clone()), advertised)
            .await?;
        // Persistence may await while the attempt is fenced/replaced. Revoke
        // before exposing a credential whose originating port is no longer live.
        match runtime.mcp_surface_snapshot(parent_run_id, &surface) {
            Ok(Some(_)) => Ok(exchange),
            Ok(None) | Err(_) => {
                exchange.disconnect().await?;
                Err(McpChatError::Unavailable)
            }
        }
    }

    /// Tests only: counterpart of `connect`, called by `CommandPort::prepare`
    /// in production, for protocol/listener acceptance without an executing
    /// provider parent. No lineage is inferred; calls still use ordinary chat.
    #[cfg(any(test, feature = "test-support"))]
    pub async fn connect_transport_fixture(
        self: &Arc<Self>,
        parent_run_id: Uuid,
    ) -> Result<McpProviderExchange, McpChatError> {
        let advertised = self
            .discovery
            .snapshot()
            .map_err(|_| McpChatError::Unavailable)?;
        self.connect_selected(parent_run_id, None, advertised).await
    }

    async fn connect_selected(
        self: &Arc<Self>,
        parent_run_id: Uuid,
        surface: Option<CapabilitySurfaceVersion>,
        advertised: Arc<McpRecipeDiscoverySnapshot>,
    ) -> Result<McpProviderExchange, McpChatError> {
        self.prepare_startup().await?;
        if self.runtime.upgrade().is_none() {
            return Err(McpChatError::Unavailable);
        }
        advertised
            .qualification_checksum()
            .ok_or(McpChatError::Unavailable)?;
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
        let id = Uuid::new_v4();
        let qualification = hex::encode(
            advertised
                .qualification_checksum()
                .ok_or(McpChatError::Unavailable)?,
        );
        self.pool.get().await.map_err(|_| McpChatError::Storage)?
            .execute("INSERT INTO brassclaw_mcp_exchanges(exchange_id,parent_run_id,token_hash,catalogue_id,qualification_checksum,advertised_tools,owner_scope,parent_surface_version) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
                &[&id,&parent_run_id,&token_hash,&advertised.generation(),&qualification,&advertised.tools_list(),&self.owner_scope,&surface.as_ref().map(CapabilitySurfaceVersion::as_str)])
            .await.map_err(|_| McpChatError::Storage)?;
        let exchange = Arc::new(Exchange {
            id,
            parent_run_id,
            parent_surface_version: surface,
            protocol_state: AtomicU8::new(0),
            advertised,
            cancellation: CancellationToken::new(),
            calls: Mutex::new(HashMap::new()),
        });
        self.exchanges
            .lock()
            .map_err(|_| McpChatError::Unavailable)?
            .insert(token_hash.clone(), exchange.clone());
        Ok(McpProviderExchange {
            bridge: self.clone(),
            exchange,
            token,
            token_hash,
        })
    }

    pub(crate) fn advertised_snapshot(
        &self,
    ) -> Result<Arc<McpRecipeDiscoverySnapshot>, McpChatError> {
        self.discovery
            .snapshot()
            .map_err(|_| McpChatError::Unavailable)
    }

    /// Instance shutdown revokes all live credentials immediately. A provider
    /// exchange uses its own disconnect and never calls this instance method.
    pub fn revoke_all(&self) {
        if let Ok(mut exchanges) = self.exchanges.lock() {
            for exchange in exchanges.values() {
                exchange.cancellation.cancel();
            }
            exchanges.clear();
        }
    }

    pub async fn disconnect_all(&self) -> Result<(), McpChatError> {
        self.revoke_all();
        self.pool.get().await.map_err(|_| McpChatError::Storage)?
            .execute("UPDATE brassclaw_mcp_exchanges SET active=false,disconnected_at=COALESCE(disconnected_at,clock_timestamp()) WHERE owner_scope=$1 AND active", &[&self.owner_scope])
            .await.map_err(|_| McpChatError::Storage)?;
        Ok(())
    }

    /// Startup invalidates old provider credentials and reconciles only already
    /// terminal original runs. Missing/unfinished admissions remain unresolved.
    /// No pending command is resubmitted here.
    pub async fn prepare_startup(&self) -> Result<usize, McpChatError> {
        let mut prepared = self.startup_prepared.lock().await;
        if *prepared {
            return Ok(0);
        }
        let runtime = self.runtime.upgrade().ok_or(McpChatError::Unavailable)?;
        let client = self.pool.get().await.map_err(|_| McpChatError::Storage)?;
        client.execute("UPDATE brassclaw_mcp_exchanges SET active=false,disconnected_at=COALESCE(disconnected_at,clock_timestamp()) WHERE owner_scope=$1 AND active", &[&self.owner_scope])
            .await.map_err(|_| McpChatError::Storage)?;
        let rows = client.query("SELECT c.call_id,c.phase,c.terminal_response,c.command FROM brassclaw_mcp_chat_calls c JOIN brassclaw_mcp_exchanges e USING(exchange_id) WHERE e.owner_scope=$1 AND c.phase IN(1,2)", &[&self.owner_scope])
            .await.map_err(|_| McpChatError::Storage)?;
        drop(client);
        let mut unresolved = 0;
        for row in rows {
            let call_id: Uuid = row.get(0);
            let conversation = crate::runtime::ConversationId(
                brassclaw_host_api::ThreadId::new(format!("reborn-conv-{call_id}"))
                    .map_err(|_| McpChatError::Storage)?,
            );
            if row.get::<_, i16>(1) == 1 {
                let reply = match runtime
                    .recover_correlated_user_message(&conversation, call_id)
                    .await
                    .map_err(|_| McpChatError::Storage)?
                {
                    Some(reply) => reply,
                    None => {
                        unresolved += 1;
                        continue;
                    }
                };
                let recorded = self
                    .record_terminal(
                        &runtime,
                        &conversation,
                        call_id,
                        row.get::<_, String>(3).as_str(),
                        reply,
                    )
                    .await;
                match recorded {
                    Ok(_) => {}
                    Err(McpChatError::Unresolved) => {
                        unresolved += 1;
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            let terminal = self
                .pool
                .get()
                .await
                .map_err(|_| McpChatError::Storage)?
                .query_one(
                    "SELECT terminal_response,accepted_message_ref,reply_message_ref
                    FROM brassclaw_mcp_chat_calls WHERE call_id=$1 AND phase IN(2,3)",
                    &[&call_id],
                )
                .await
                .map_err(|_| McpChatError::Storage)?;
            let response: Value = terminal.get(0);
            let user_ref: Option<String> = terminal.get(1);
            let reply_ref: Option<String> = terminal.get(2);
            match self
                .verify_cached(
                    &runtime,
                    &conversation,
                    call_id,
                    row.get::<_, String>(3).as_str(),
                    CachedReceipt {
                        response: &response,
                        user_ref: user_ref.as_deref(),
                        reply_ref: reply_ref.as_deref(),
                    },
                )
                .await
            {
                Ok(()) => {}
                Err(McpChatError::Unresolved) => {
                    unresolved += 1;
                    continue;
                }
                Err(error) => return Err(error),
            }
            runtime
                .close_correlated_conversation(&conversation, call_id)
                .await
                .map_err(|_| McpChatError::Storage)?;
            self.pool.get().await.map_err(|_| McpChatError::Storage)?.execute("UPDATE brassclaw_mcp_chat_calls SET phase=3,updated_at=clock_timestamp() WHERE call_id=$1 AND phase=2", &[&call_id])
                .await.map_err(|_| McpChatError::Storage)?;
        }
        if unresolved > 0 {
            tracing::warn!(
                unresolved,
                "MCP calls remain unresolved; original evidence is retained without replay"
            );
        }
        *prepared = true;
        Ok(unresolved)
    }

    async fn record_terminal(
        &self,
        runtime: &RebornRuntime,
        conversation: &crate::runtime::ConversationId,
        call_id: Uuid,
        command: &str,
        reply: crate::runtime::AssistantReply,
    ) -> Result<Value, McpChatError> {
        let unsubmitted = self.verify_terminal_subject(call_id, command).await?;
        let (accepted_message_ref, reply_message_ref) = runtime
            .correlated_chat_message_refs(conversation, call_id, command)
            .await
            .map_err(McpChatError::from)?;
        if let Some(original_user) = unsubmitted
            && (!reply.status.is_terminal()
                || reply.is_successful_final_reply()
                || reply.text.is_some()
                || reply_message_ref.is_some()
                || original_user != accepted_message_ref)
        {
            return Err(McpChatError::Storage);
        }
        let response = if reply.is_successful_final_reply() {
            json!({"content":[{"type":"text","text":reply.text}],"isError":false})
        } else {
            json!({"content":[{"type":"text","text":"The original ordinary chat did not complete successfully."}],"isError":true})
        };
        let client = self.pool.get().await.map_err(|_| McpChatError::Storage)?;
        let changed = client.execute("UPDATE brassclaw_mcp_chat_calls SET phase=2,terminal_response=$2,accepted_message_ref=$3,reply_message_ref=$4,updated_at=clock_timestamp() WHERE call_id=$1 AND phase=1", &[&call_id,&response,&accepted_message_ref,&reply_message_ref])
                .await.map_err(|_| McpChatError::Storage)?;
        if changed == 0 {
            let stored: Value = client
                .query_one(
                    "SELECT terminal_response FROM brassclaw_mcp_chat_calls WHERE call_id=$1",
                    &[&call_id],
                )
                .await
                .map_err(|_| McpChatError::Storage)?
                .get(0);
            if stored != response {
                return Err(McpChatError::Storage);
            }
        }
        Ok(response)
    }

    async fn verify_cached(
        &self,
        runtime: &RebornRuntime,
        conversation: &crate::runtime::ConversationId,
        call_id: Uuid,
        command: &str,
        receipt: CachedReceipt<'_>,
    ) -> Result<(), McpChatError> {
        let unsubmitted = self.verify_terminal_subject(call_id, command).await?;
        let reply = runtime
            .recover_correlated_user_message(conversation, call_id)
            .await
            .map_err(|_| McpChatError::Storage)?
            .ok_or(McpChatError::Unresolved)?;
        let (actual_user, actual_reply) = runtime
            .correlated_chat_message_refs(conversation, call_id, command)
            .await
            .map_err(McpChatError::from)?;
        if let Some(original_user) = unsubmitted
            && (!reply.status.is_terminal()
                || reply.is_successful_final_reply()
                || reply.text.is_some()
                || actual_reply.is_some()
                || original_user != actual_user)
        {
            return Err(McpChatError::Storage);
        }
        let expected = if reply.is_successful_final_reply() {
            json!({"content":[{"type":"text","text":reply.text}],"isError":false})
        } else {
            json!({"content":[{"type":"text","text":"The original ordinary chat did not complete successfully."}],"isError":true})
        };
        if &expected != receipt.response
            || receipt.user_ref != Some(actual_user.as_str())
            || actual_reply.as_deref() != receipt.reply_ref
        {
            return Err(McpChatError::Storage);
        }
        Ok(())
    }

    /// Definitive pre-VM rejection/cancellation is a terminal failure without
    /// a Recipe or reply. Verify the real durable admission and absence of
    /// execution; neither an ordinary failure status nor missing selection is
    /// sufficient. Other terminals retain the full existing matcher proof.
    async fn verify_terminal_subject(
        &self,
        call_id: Uuid,
        command: &str,
    ) -> Result<Option<String>, McpChatError> {
        let client = self.pool.get().await.map_err(|_| McpChatError::Storage)?;
        let row = client.query_opt("SELECT a.outcome,a.accepted_message_ref,
            EXISTS(SELECT 1 FROM brassclaw_monty_recipe_selections s WHERE s.run_id=a.run_id)
            OR EXISTS(SELECT 1 FROM brassclaw_monty_tool_invocations i WHERE i.run_id=a.run_id)
            FROM brassclaw_mcp_chat_calls c JOIN brassclaw_monty_task_admissions a ON a.run_id=c.call_id
            WHERE c.call_id=$1 AND c.owner_scope=$2 AND c.command=$3
                AND a.phase='settled' AND a.started_at IS NULL",
            &[&call_id,&self.owner_scope,&command]).await.map_err(|_| McpChatError::Storage)?;
        if let Some(row) = row {
            let outcome: Value = row.get(0);
            let reason = outcome.get("reason_kind").and_then(Value::as_str);
            if outcome.as_object().is_some_and(|object| object.len() == 2)
                && outcome.get("status").and_then(Value::as_str) == Some("failed")
                && matches!(
                    reason,
                    Some(
                        "task_cancelled"
                            | "monty_input_rejected"
                            | "monty_admission_backpressure"
                            | "monty_service_closed"
                            | "monty_admission_rejected"
                    )
                )
                && !row.get::<_, bool>(2)
            {
                return Ok(Some(row.get(1)));
            }
            return Err(McpChatError::Unresolved);
        }
        drop(client);
        self.verify_matched_subject(call_id, command).await?;
        Ok(None)
    }

    // Observe the original ordinary matcher and pinned selection. This never
    // selects a Recipe, invokes IBS or replays a command during recovery.
    async fn verify_matched_subject(
        &self,
        call_id: Uuid,
        command: &str,
    ) -> Result<(), McpChatError> {
        let client = self.pool.get().await.map_err(|_| McpChatError::Storage)?;
        let row = client.query_opt("SELECT a.outcome,q.evidence_bytes,c.tool_name,s.recipe_id FROM brassclaw_mcp_chat_calls c JOIN brassclaw_mcp_exchanges e USING(exchange_id) JOIN brassclaw_mcp_command_qualifications q ON q.checksum=e.qualification_checksum AND q.catalogue_id=e.catalogue_id JOIN brassclaw_monty_task_admissions a ON a.run_id=c.call_id JOIN brassclaw_monty_recipe_selections s ON s.run_id=c.call_id AND s.selection_checksum=(q.evidence_bytes::jsonb->'commands'->c.tool_name->>'selection_checksum') AND s.selection_checksum=encode(sha256(convert_to(s.selection_bytes,'UTF8')),'hex') WHERE c.call_id=$1 AND c.owner_scope=$2 AND c.command=$3 AND a.phase='settled'", &[&call_id,&self.owner_scope,&command])
            .await.map_err(|_| McpChatError::Storage)?.ok_or(McpChatError::Unresolved)?;
        let outcome: Value = row.get(0);
        let evidence: Value =
            serde_json::from_str(&row.get::<_, String>(1)).map_err(|_| McpChatError::Storage)?;
        let name: String = row.get(2);
        let selected = evidence["commands"][&name]["selection_checksum"]
            .as_str()
            .ok_or(McpChatError::Storage)?;
        let recipes = outcome["execution"]["recipes"]
            .as_array()
            .ok_or(McpChatError::Unresolved)?;
        let matched: Vec<_> = recipes
            .iter()
            .filter(|r| !r["normal_match"].is_null())
            .collect();
        if outcome["execution"]["intent_outcome"] != "match" || matched.len() != 1 {
            return Err(McpChatError::Unresolved);
        }
        let recipe = matched[0];
        if recipe["recipe_id"] != row.get::<_, Uuid>(3).to_string()
            || recipe["normal_match"]["matching"]["recipe_uuid"] != recipe["recipe_id"]
            || recipe["selection_checksum"] != selected
            || recipe["normal_match"]["matching"]["selection_checksum"] != selected
            || recipe["normal_match"]["matching"]["command_checksum"]
                != hex::encode(Sha256::digest(command.as_bytes()))
        {
            return Err(McpChatError::Unresolved);
        }
        Ok(())
    }

    fn authenticate(&self, token: &str) -> Result<Arc<Exchange>, McpChatError> {
        if token.len() != 64 {
            return Err(McpChatError::Unavailable);
        }
        let key = format!("{:x}", Sha256::digest(token.as_bytes()));
        let exchange = self
            .exchanges
            .lock()
            .map_err(|_| McpChatError::Unavailable)?
            .get(&key)
            .cloned()
            .ok_or(McpChatError::Unavailable)?;
        if exchange.cancellation.is_cancelled() {
            return Err(McpChatError::Unavailable);
        }
        if let Some(surface) = &exchange.parent_surface_version {
            let live = self
                .runtime
                .upgrade()
                .ok_or(McpChatError::Unavailable)?
                .mcp_surface_snapshot(exchange.parent_run_id, surface)
                .map_err(|_| McpChatError::Unavailable)?
                .is_some();
            if !live {
                exchange.cancellation.cancel();
                return Err(McpChatError::Unavailable);
            }
        }
        Ok(exchange)
    }

    pub fn authenticate_exchange(&self, token: &str) -> Result<Uuid, McpChatError> {
        Ok(self.authenticate(token)?.id)
    }

    pub fn cancel_request(&self, token: &str, request_id: &Value) -> Result<(), McpChatError> {
        let exchange = self.authenticate(token)?;
        let key = request_key(request_id)?;
        if let Some(call) = exchange
            .calls
            .lock()
            .map_err(|_| McpChatError::Unavailable)?
            .get(&key)
        {
            call.call.cancellation.cancel();
        }
        Ok(())
    }

    pub fn initialize(&self, token: &str) -> Result<Uuid, McpChatError> {
        let exchange = self.authenticate(token)?;
        exchange
            .protocol_state
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| McpChatError::RequestConflict)?;
        Ok(exchange.id)
    }

    pub fn initialized(&self, token: &str) -> Result<(), McpChatError> {
        let exchange = self.authenticate(token)?;
        exchange
            .protocol_state
            .compare_exchange(1, 2, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| McpChatError::RequestConflict)?;
        Ok(())
    }

    pub fn tools_list(&self, token: &str) -> Result<Value, McpChatError> {
        let exchange = self.authenticate(token)?;
        if exchange.protocol_state.load(Ordering::SeqCst) < 2 {
            return Err(McpChatError::Unavailable);
        }
        exchange.protocol_state.store(3, Ordering::SeqCst);
        Ok(exchange.advertised.tools_list())
    }

    /// Execute only the winner of the durable pre-submission reservation.
    /// Repeated/unknown requests read the original run; they never resubmit it.
    pub async fn call(
        &self,
        token: &str,
        request_id: &Value,
        name: &str,
        command: &str,
    ) -> Result<Value, McpChatError> {
        if !request_id.is_string() && !request_id.is_number() {
            return Err(McpChatError::RequestConflict);
        }
        let exchange = self.authenticate(token)?;
        if exchange.protocol_state.load(Ordering::SeqCst) != 3 {
            return Err(McpChatError::Unavailable);
        }
        let command_checksum = hex::encode(
            exchange
                .advertised
                .validate_command(name, command)
                .map_err(|_| McpChatError::InvalidCommand)?,
        );
        let lease = self
            .acquire_call(&exchange, request_key(request_id)?)
            .await?;
        let cancellation = &lease.call.cancellation;
        let runtime = self.runtime.upgrade().ok_or(McpChatError::Unavailable)?;
        let mut client = self.pool.get().await.map_err(|_| McpChatError::Storage)?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| McpChatError::Storage)?;
        let existing = tx.query_opt("SELECT call_id FROM brassclaw_mcp_chat_calls WHERE parent_run_id=$1 AND request_id_digest=brassclaw_mcp_request_id_digest($2) AND request_id=$2 AND owner_scope=$3", &[&exchange.parent_run_id,request_id,&self.owner_scope])
            .await.map_err(|_| McpChatError::Storage)?;
        if existing.is_none() {
            self.discovery
                .validate_advertised_command(&exchange.advertised, name, command)
                .map_err(|_| McpChatError::InvalidCommand)?;
        }
        let call_id = Uuid::new_v4();
        tx.execute("INSERT INTO brassclaw_mcp_chat_calls(call_id,exchange_id,request_id,tool_name,command,parent_run_id,owner_scope,command_checksum) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(parent_run_id,owner_scope,request_id_digest) DO NOTHING",
            &[&call_id,&exchange.id,request_id,&name,&command,&exchange.parent_run_id,&self.owner_scope,&command_checksum]).await.map_err(|_| McpChatError::Storage)?;
        let row = tx.query_one("SELECT call_id,tool_name,command,phase,terminal_response,command_checksum,accepted_message_ref,reply_message_ref FROM brassclaw_mcp_chat_calls WHERE parent_run_id=$1 AND request_id_digest=brassclaw_mcp_request_id_digest($2) AND request_id=$2 AND owner_scope=$3 FOR UPDATE", &[&exchange.parent_run_id,request_id,&self.owner_scope])
            .await.map_err(|_| McpChatError::Storage)?;
        if row.get::<_, String>(1) != name || row.get::<_, String>(2) != command {
            return Err(McpChatError::RequestConflict);
        }
        if row.get::<_, String>(5) != command_checksum {
            return Err(McpChatError::InvalidCommand);
        }
        let call_id: Uuid = row.get(0);
        let phase: i16 = row.get(3);
        let saved: Option<Value> = row.get(4);
        if phase == 0 {
            if cancellation.is_cancelled() {
                return Err(McpChatError::Unavailable);
            }
            self.discovery
                .validate_advertised_command(&exchange.advertised, name, command)
                .map_err(|_| McpChatError::InvalidCommand)?;
            tx.execute("UPDATE brassclaw_mcp_chat_calls SET phase=1,updated_at=clock_timestamp() WHERE call_id=$1", &[&call_id]).await.map_err(|_| McpChatError::Storage)?;
        }
        tx.commit().await.map_err(|_| McpChatError::Storage)?;
        drop(client);
        // ID allocation was durably recorded before chat creation/admission.
        let conversation = crate::runtime::ConversationId(
            brassclaw_host_api::ThreadId::new(format!("reborn-conv-{call_id}"))
                .map_err(|_| McpChatError::Storage)?,
        );
        if phase >= 2 {
            let user_ref: Option<String> = row.get(6);
            let reply_ref: Option<String> = row.get(7);
            self.verify_cached(
                &runtime,
                &conversation,
                call_id,
                command,
                CachedReceipt {
                    response: saved.as_ref().ok_or(McpChatError::Storage)?,
                    user_ref: user_ref.as_deref(),
                    reply_ref: reply_ref.as_deref(),
                },
            )
            .await?;
        }
        if phase == 3 {
            if cancellation.is_cancelled() {
                return Err(McpChatError::Unavailable);
            }
            return saved.ok_or(McpChatError::Storage);
        }
        let response = if phase == 2 {
            saved.ok_or(McpChatError::Storage)?
        } else {
            let reply = if phase == 0 {
                if cancellation.is_cancelled() {
                    return Err(McpChatError::Unresolved);
                }
                runtime
                    .ensure_correlated_conversation(call_id)
                    .await
                    .map_err(|_| McpChatError::Unresolved)?;
                runtime
                    .send_correlated_user_message(
                        &conversation,
                        command,
                        call_id,
                        cancellation.child_token(),
                    )
                    .await
                    .map_err(|_| McpChatError::Unresolved)?
            } else {
                runtime
                    .recover_correlated_user_message(&conversation, call_id)
                    .await
                    .map_err(|_| McpChatError::Storage)?
                    .ok_or(McpChatError::Unresolved)?
            };
            self.record_terminal(&runtime, &conversation, call_id, command, reply)
                .await?
        };
        runtime
            .close_correlated_conversation(&conversation, call_id)
            .await
            .map_err(|_| McpChatError::Unresolved)?;
        self.pool.get().await.map_err(|_| McpChatError::Storage)?
            .execute("UPDATE brassclaw_mcp_chat_calls SET phase=3,updated_at=clock_timestamp() WHERE call_id=$1 AND phase=2", &[&call_id]).await.map_err(|_| McpChatError::Storage)?;
        if cancellation.is_cancelled() {
            return Err(McpChatError::Unavailable);
        }
        Ok(response)
    }
}

#[cfg(test)]
pub(crate) async fn assert_native_chat_transport(runtime: Arc<RebornRuntime>, parent_run_id: Uuid) {
    use crate::orchestrator_mcp_server::{
        OrchestratorMcpServerConfig, orchestrator_mcp_router_with_chat,
    };
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn post(
        router: axum::Router,
        token: &str,
        session: Option<Uuid>,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("authorization", format!("Bearer {token}"));
        if let Some(session) = session {
            builder = builder
                .header("mcp-session-id", session.to_string())
                .header("mcp-protocol-version", "2025-06-18");
        }
        let response = router
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (
            status,
            if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).unwrap()
            },
        )
    }
    async fn initialize(router: axum::Router, exchange: &McpProviderExchange) {
        let (status, result) = post(router.clone(), exchange.bearer_token(), None,
            json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"native-acceptance","version":"1"}}})).await;
        assert_eq!(status, StatusCode::OK);
        assert!(result.get("error").is_none());
        assert_eq!(
            post(
                router.clone(),
                exchange.bearer_token(),
                Some(exchange.id()),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"})
            )
            .await
            .0,
            StatusCode::ACCEPTED
        );
        let (_, listed) = post(
            router,
            exchange.bearer_token(),
            Some(exchange.id()),
            json!({"jsonrpc":"2.0","id":"list","method":"tools/list"}),
        )
        .await;
        assert_eq!(listed["result"], exchange.tools_list());
    }
    let bridge = runtime.mcp_chat_bridge().unwrap();
    assert!(Arc::ptr_eq(&bridge, &runtime.mcp_chat_bridge().unwrap()));
    let first = bridge
        .connect_transport_fixture(parent_run_id)
        .await
        .unwrap();
    let second_parent: Uuid = bridge.pool.get().await.unwrap().query_one(
        "SELECT run_id FROM brassclaw_monty_task_admissions WHERE run_id<>$1 AND phase='settled' LIMIT 1", &[&parent_run_id],
    ).await.unwrap().get(0);
    let second = bridge
        .connect_transport_fixture(second_parent)
        .await
        .unwrap();
    assert_ne!(first.id(), second.id());
    assert_ne!(first.bearer_token(), second.bearer_token());
    let router =
        orchestrator_mcp_router_with_chat(OrchestratorMcpServerConfig::default(), bridge.clone());
    assert_eq!(
        post(
            router.clone(),
            "invalid",
            None,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    initialize(router.clone(), &first).await;
    initialize(router.clone(), &second).await;
    let settings = runtime.webui_monty_settings_store();
    async fn set_capacity(
        settings: &Arc<dyn brassclaw_product_workflow::MontyVmSettingsStore>,
        capacity: u32,
    ) {
        let current = settings.get("default", "default").await.unwrap();
        let mut limits = current.execution_limits;
        limits.max_mcp_inflight_calls = capacity;
        settings
            .upsert(
                "default",
                "default",
                &serde_json::from_value(json!({
                    "expected_revision": current.revision, "execution_limits": limits,
                }))
                .unwrap(),
            )
            .await
            .unwrap();
    }
    let previous_capacity = runtime
        .acknowledged_monty_execution_limits()
        .await
        .unwrap()
        .max_mcp_inflight_calls;
    set_capacity(&settings, 1).await;
    // Durable JSONB deduplication and in-memory cancellation must identify the
    // same request, including alternate numeric spellings and signed zero.
    let client = bridge.pool.get().await.unwrap();
    for (left, right) in [
        (json!(1), json!(1.0)),
        (json!(1000), json!(1e3)),
        (json!(-0.0), json!(0)),
        (json!(0.001), json!(1e-3)),
        (json!(10_000_000_000_000_000_000_u64), json!(1e19)),
    ] {
        let equality = client
            .query_one("SELECT $1::jsonb=$2::jsonb, brassclaw_mcp_request_id_digest($1)=brassclaw_mcp_request_id_digest($2)", &[&left, &right])
            .await
            .unwrap();
        assert!(
            equality.get::<_, bool>(0),
            "numeric fixture must be equal in the actual store"
        );
        assert!(
            equality.get::<_, bool>(1),
            "durable digest must preserve numeric equality"
        );
        let lease = bridge
            .acquire_call(&first.exchange, request_key(&left).unwrap())
            .await
            .unwrap();
        let duplicate = bridge
            .acquire_call(&first.exchange, request_key(&right).unwrap())
            .await
            .unwrap();
        assert!(Arc::ptr_eq(&lease.call, &duplicate.call));
        bridge.cancel_request(first.bearer_token(), &right).unwrap();
        assert!(lease.call.cancellation.is_cancelled());
        drop(duplicate);
        assert_eq!(first.exchange.calls.lock().unwrap().len(), 1);
        drop(lease);
        assert!(first.exchange.calls.lock().unwrap().is_empty());
    }
    for (left, right) in [
        (json!("1"), json!(1)),
        (json!(1.0000000000000002), json!(1)),
        (json!(u64::MAX), json!(18_446_744_073_709_551_616.0)),
        (json!(1e308), json!(1e307)),
        (json!(5e-324), json!(0)),
    ] {
        let equality = client
            .query_one("SELECT $1::jsonb=$2::jsonb, brassclaw_mcp_request_id_digest($1)=brassclaw_mcp_request_id_digest($2)", &[&left, &right])
            .await
            .unwrap();
        assert!(
            !equality.get::<_, bool>(0),
            "distinct fixture must be distinct in the actual store"
        );
        assert!(
            !equality.get::<_, bool>(1),
            "distinct fixture must have distinct durable digests"
        );
        assert_ne!(request_key(&left).unwrap(), request_key(&right).unwrap());
    }
    drop(client);
    // Real exchange ownership, independent of task/Tool execution: dropping a
    // duplicate HTTP waiter must not cancel the remaining waiter. Dropping the
    // last one must cancel admitted work even when its future was aborted.
    let lease = bridge
        .acquire_call(&first.exchange, "lease-check".into())
        .await
        .unwrap();
    let duplicate = bridge
        .acquire_call(&first.exchange, "lease-check".into())
        .await
        .unwrap();
    assert!(matches!(
        bridge
            .acquire_call(&first.exchange, "second-active".into())
            .await,
        Err(McpChatError::Unavailable)
    ));
    set_capacity(&settings, 2).await;
    let second_active = bridge
        .acquire_call(&first.exchange, "second-active".into())
        .await
        .unwrap();
    set_capacity(&settings, 1).await;
    assert!(!second_active.call.cancellation.is_cancelled());
    assert!(matches!(
        bridge
            .acquire_call(&first.exchange, "third-active".into())
            .await,
        Err(McpChatError::Unavailable)
    ));
    let debt_duplicate = bridge
        .acquire_call(&first.exchange, "lease-check".into())
        .await
        .unwrap();
    drop(debt_duplicate);
    drop(second_active);
    let cancellation = lease.call.cancellation.clone();
    // A Drop implementation can finish while its fields are still retained
    // on another thread. Such Arc references are not additional HTTP waiters.
    let retained_state = lease.call.clone();
    drop(lease);
    assert!(!cancellation.is_cancelled());
    assert_eq!(first.exchange.calls.lock().unwrap().len(), 1);
    drop(duplicate);
    assert!(cancellation.is_cancelled());
    assert!(first.exchange.calls.lock().unwrap().is_empty());
    drop(retained_state);
    for _ in 0..128 {
        let left = bridge
            .acquire_call(&first.exchange, "concurrent-drop".into())
            .await
            .unwrap();
        let right = bridge
            .acquire_call(&first.exchange, "concurrent-drop".into())
            .await
            .unwrap();
        let cancellation = left.call.cancellation.clone();
        let barrier = std::sync::Barrier::new(3);
        std::thread::scope(|scope| {
            let left_barrier = &barrier;
            let right_barrier = &barrier;
            let left_drop = scope.spawn(move || {
                left_barrier.wait();
                drop(left);
            });
            let right_drop = scope.spawn(move || {
                right_barrier.wait();
                drop(right);
            });
            barrier.wait();
            left_drop.join().unwrap();
            right_drop.join().unwrap();
        });
        assert!(cancellation.is_cancelled());
        assert!(first.exchange.calls.lock().unwrap().is_empty());
    }
    let exchange = first.exchange.clone();
    let abort_bridge = bridge.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let waiter = tokio::spawn(async move {
        let lease = abort_bridge
            .acquire_call(&exchange, "aborted-waiter".into())
            .await
            .unwrap();
        started.send(lease.call.cancellation.clone()).unwrap();
        std::future::pending::<()>().await;
        drop(lease);
    });
    let cancellation = ready.await.unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert!(cancellation.is_cancelled());
    assert!(first.exchange.calls.lock().unwrap().is_empty());
    // Completed identities consume no capacity, including long exchanges that
    // exceed the previous lifetime-map ceiling.
    for index in 0..4100 {
        drop(
            bridge
                .acquire_call(&first.exchange, format!("finished-{index}"))
                .await
                .unwrap(),
        );
    }
    assert!(first.exchange.calls.lock().unwrap().is_empty());
    set_capacity(&settings, 4100).await;
    let mut active = Vec::new();
    for index in 0..4097 {
        active.push(
            bridge
                .acquire_call(&first.exchange, format!("active-{index}"))
                .await
                .unwrap(),
        );
    }
    assert_eq!(first.exchange.calls.lock().unwrap().len(), 4097);
    set_capacity(&settings, 1).await;
    assert!(matches!(
        bridge
            .acquire_call(&first.exchange, "capacity-debt".into())
            .await,
        Err(McpChatError::Unavailable)
    ));
    assert!(
        active
            .iter()
            .all(|lease| !lease.call.cancellation.is_cancelled())
    );
    drop(active);
    assert_eq!(first.exchange.calls.lock().unwrap().capacity(), 0);
    set_capacity(&settings, previous_capacity).await;
    let a = json!({"jsonrpc":"2.0","id":"same-id","method":"tools/call","params":{"name":"publish_literal_reply","arguments":{"command":"publish literal reply transport α"}}});
    let b = json!({"jsonrpc":"2.0","id":"same-id","method":"tools/call","params":{"name":"publish_literal_reply","arguments":{"command":"publish literal reply transport β"}}});
    let ((_, result_a), (_, result_b)) = tokio::join!(
        post(
            router.clone(),
            first.bearer_token(),
            Some(first.id()),
            a.clone()
        ),
        post(
            router.clone(),
            second.bearer_token(),
            Some(second.id()),
            b.clone()
        )
    );
    assert_eq!(result_a["result"]["content"][0]["text"], "transport α");
    assert_eq!(result_b["result"]["content"][0]["text"], "transport β");
    let reconnected = bridge
        .connect_transport_fixture(parent_run_id)
        .await
        .unwrap();
    initialize(router.clone(), &reconnected).await;
    assert_eq!(
        post(
            router.clone(),
            reconnected.bearer_token(),
            Some(reconnected.id()),
            a.clone()
        )
        .await
        .1,
        result_a
    );
    reconnected.disconnect().await.unwrap();
    assert_eq!(
        post(router.clone(), first.bearer_token(), Some(first.id()), a)
            .await
            .1,
        result_a
    );
    assert_eq!(
        post(router.clone(), first.bearer_token(), Some(first.id()), b)
            .await
            .1["error"]["code"],
        -32602
    );
    assert!(first.exchange.calls.lock().unwrap().is_empty());
    assert!(second.exchange.calls.lock().unwrap().is_empty());
    let client = bridge.pool.get().await.unwrap();
    let rows = client.query("SELECT call_id,phase,accepted_message_ref,reply_message_ref FROM brassclaw_mcp_chat_calls WHERE exchange_id IN($1,$2)", &[&first.id(),&second.id()]).await.unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let call_id: Uuid = row.get(0);
        assert_eq!(row.get::<_, i16>(1), 3);
        assert!(row.get::<_, Option<String>>(2).unwrap().starts_with("msg:"));
        assert!(row.get::<_, Option<String>>(3).unwrap().starts_with("msg:"));
        let chat = format!("reborn-conv-{call_id}");
        let metadata: Value = client
            .query_one(
                "SELECT metadata FROM brassclaw_session_threads WHERE id=$1",
                &[&chat],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(metadata["closed"], true);
        assert_eq!(
            metadata["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["kind"] == "assistant" && m["status"] == "finalized")
                .count(),
            1
        );
        let outcome: Value = client.query_one("SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'", &[&call_id]).await.unwrap().get(0);
        assert_eq!(outcome["execution"]["intent_outcome"], "match");
        assert!(!outcome["execution"]["recipes"][0]["normal_match"].is_null());
    }
    // An uncertain send never gets a replacement chat or a second submission.
    let unknown = Uuid::new_v4();
    client.execute("INSERT INTO brassclaw_mcp_chat_calls(call_id,exchange_id,request_id,tool_name,command,phase,parent_run_id,owner_scope,command_checksum) VALUES($1,$2,$3,'publish_literal_reply','publish literal reply unknown',1,$4,$5,$6)", &[&unknown,&second.id(),&json!("unknown"),&second_parent,&bridge.owner_scope,&hex::encode(second.exchange.advertised.validate_command("publish_literal_reply","publish literal reply unknown").unwrap())]).await.unwrap();
    assert!(matches!(
        bridge
            .call(
                second.bearer_token(),
                &json!("unknown"),
                "publish_literal_reply",
                "publish literal reply unknown"
            )
            .await,
        Err(McpChatError::Unresolved)
    ));
    let chat = format!("reborn-conv-{unknown}");
    assert!(
        client
            .query_opt(
                "SELECT id FROM brassclaw_session_threads WHERE id=$1",
                &[&chat]
            )
            .await
            .unwrap()
            .is_none()
    );
    let first_token = first.bearer_token().to_owned();
    assert_eq!(post(router.clone(), first.bearer_token(), Some(first.id()),
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"same-id"}})).await.0, StatusCode::ACCEPTED);
    // A late cancellation cannot poison a completed durable receipt. Only
    // active requests live in the cancellation index.
    assert!(first.exchange.calls.lock().unwrap().is_empty());
    assert_eq!(
        bridge
            .call(
                first.bearer_token(),
                &json!("same-id"),
                "publish_literal_reply",
                "publish literal reply transport α"
            )
            .await
            .unwrap(),
        result_a["result"]
    );
    let first_id = first.id();
    first.disconnect().await.unwrap();
    assert_eq!(
        post(
            router.clone(),
            &first_token,
            Some(first_id),
            json!({"jsonrpc":"2.0","id":"list","method":"tools/list"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            router,
            second.bearer_token(),
            Some(second.id()),
            json!({"jsonrpc":"2.0","id":"list2","method":"tools/list"})
        )
        .await
        .0,
        StatusCode::OK
    );
    second.disconnect().await.unwrap();
}
