//! Request-local Kohai MCP sessions. The existing Monty root owns Tool-call
//! sequencing; this adapter only advertises commands and exchanges MCP packets.
use crate::mcp_chat_bridge::{McpChatBridge, McpProviderExchange};
use crate::mcp_recipe_catalogue::McpRecipeDiscoverySnapshot;
use async_trait::async_trait;
use brassclaw_host_api::{CapabilityId, InvocationId, RuntimeKind};
use brassclaw_loop_support::{
    CapabilityResultWrite, HostManagedModelError, HostManagedModelErrorKind,
    HostManagedModelGateway, HostManagedModelRequest, HostManagedModelResponse,
    LoopCapabilityPortFactory, LoopCapabilityResultWriter,
};
use brassclaw_turns::{TurnRunId, run_profile::*};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock, Weak},
};

fn unavailable() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Unavailable,
        "request-local MCP transport is unavailable",
    )
}
fn invalid() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::InvalidInvocation,
        "MCP command is outside this advertised exchange",
    )
}

#[derive(Default)]
pub(crate) struct McpProviderBinding {
    endpoint: RwLock<Option<(Weak<McpChatBridge>, String)>>,
    ports: Mutex<HashMap<TurnRunId, Weak<CommandPort>>>,
}
impl McpProviderBinding {
    pub(crate) fn activate(
        &self,
        bridge: &Arc<McpChatBridge>,
        endpoint: &str,
    ) -> Result<(), AgentLoopHostError> {
        let url = reqwest::Url::parse(endpoint).map_err(|_| invalid())?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.port().is_none()
            || url.path() != "/mcp"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(invalid());
        }
        let mut current = self.endpoint.write().map_err(|_| unavailable())?;
        if current.is_some() {
            return Err(invalid());
        }
        *current = Some((Arc::downgrade(bridge), endpoint.to_owned()));
        Ok(())
    }
    fn endpoint(&self) -> Result<Option<(Arc<McpChatBridge>, String)>, AgentLoopHostError> {
        let current = self.endpoint.read().map_err(|_| unavailable())?;
        match current.as_ref() {
            Some((bridge, url)) => Ok(Some((
                bridge.upgrade().ok_or_else(unavailable)?,
                url.clone(),
            ))),
            None => Ok(None),
        }
    }
    pub(crate) fn port(
        &self,
        run_id: TurnRunId,
    ) -> Result<Option<Arc<CommandPort>>, AgentLoopHostError> {
        Ok(self
            .ports
            .lock()
            .map_err(|_| unavailable())?
            .get(&run_id)
            .and_then(Weak::upgrade))
    }
    pub(crate) async fn finish_port(&self, port: &Arc<CommandPort>) {
        port.revoke_now();
        if let Ok(mut ports) = self.ports.lock()
            && ports
                .get(&port.context.run_id)
                .and_then(Weak::upgrade)
                .is_some_and(|current| Arc::ptr_eq(&current, port))
        {
            ports.remove(&port.context.run_id);
        }
        if let Err(error) = port.finish().await {
            tracing::debug!(reason=%error,"MCP exchange durable disconnect failed; credential is revoked");
        }
    }
}

pub(crate) struct McpCommandPortFactory {
    pub(crate) inner: Arc<dyn LoopCapabilityPortFactory>,
    pub(crate) binding: Arc<McpProviderBinding>,
    pub(crate) writer: Arc<dyn LoopCapabilityResultWriter>,
}
#[async_trait]
impl LoopCapabilityPortFactory for McpCommandPortFactory {
    async fn create_capability_port(
        &self,
        context: &LoopRunContext,
    ) -> Result<Arc<dyn LoopCapabilityPort>, AgentLoopHostError> {
        let Some((bridge, endpoint)) = self.binding.endpoint()? else {
            return self.inner.create_capability_port(context).await;
        };
        let snapshot = bridge.advertised_snapshot().map_err(|_| unavailable())?;
        let list = snapshot.tools_list();
        let entries = list["tools"].as_array().ok_or_else(unavailable)?;
        let mut definitions = Vec::new();
        for entry in entries {
            let name = entry["name"].as_str().ok_or_else(unavailable)?.to_owned();
            definitions.push(ProviderToolDefinition {
                capability_id: CapabilityId::new(format!("mcp.command.{name}"))
                    .map_err(|_| invalid())?,
                name,
                description: entry["description"]
                    .as_str()
                    .ok_or_else(unavailable)?
                    .to_owned(),
                parameters: entry["inputSchema"].clone(),
            });
        }
        let version = CapabilitySurfaceVersion::new(format!(
            "mcp:{}:{}",
            snapshot.generation(),
            uuid::Uuid::new_v4()
        ))
        .map_err(|_| invalid())?;
        let port = Arc::new(CommandPort {
            context: context.clone(),
            bridge,
            endpoint,
            snapshot,
            definitions,
            version,
            writer: self.writer.clone(),
            inputs: Mutex::new(HashMap::new()),
            results: tokio::sync::Mutex::new(HashMap::new()),
            connection: tokio::sync::Mutex::new(None),
            revoked: std::sync::atomic::AtomicBool::new(false),
            live_lease: Mutex::new(None),
        });
        {
            let mut ports = self.binding.ports.lock().map_err(|_| unavailable())?;
            if ports
                .get(&context.run_id)
                .and_then(Weak::upgrade)
                .is_some_and(|current| !current.revoked.load(std::sync::atomic::Ordering::SeqCst))
            {
                return Err(unavailable());
            }
            ports.insert(context.run_id, Arc::downgrade(&port));
        }
        Ok(port)
    }
}

#[derive(Clone)]
struct ConnectedExchange {
    lease: Arc<McpProviderExchange>,
    client: reqwest::Client,
    endpoint: String,
}
impl ConnectedExchange {
    async fn post(&self, body: Value, initialized: bool) -> Result<Value, AgentLoopHostError> {
        let mut request = self
            .client
            .post(&self.endpoint)
            .bearer_auth(self.lease.bearer_token())
            .header("accept", "application/json, text/event-stream")
            .json(&body);
        if initialized {
            request = request
                .header("mcp-session-id", self.lease.id().to_string())
                .header("mcp-protocol-version", "2025-06-18");
        }
        let mut response = request.send().await.map_err(|_| unavailable())?;
        if !response.status().is_success() {
            return Err(unavailable());
        }
        if body.get("id").is_none() {
            return Ok(Value::Null);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if bytes.len().saturating_add(chunk.len()) > 1024 * 1024 {
                return Err(unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
        if value["jsonrpc"] != "2.0" || value["id"] != body["id"] || value.get("error").is_some() {
            return Err(unavailable());
        }
        value.get("result").cloned().ok_or_else(unavailable)
    }
}
pub(crate) struct CommandPort {
    context: LoopRunContext,
    bridge: Arc<McpChatBridge>,
    endpoint: String,
    snapshot: Arc<McpRecipeDiscoverySnapshot>,
    definitions: Vec<ProviderToolDefinition>,
    version: CapabilitySurfaceVersion,
    writer: Arc<dyn LoopCapabilityResultWriter>,
    inputs: Mutex<HashMap<String, ProviderToolCall>>,
    results: tokio::sync::Mutex<HashMap<String, CapabilityResultMessage>>,
    connection: tokio::sync::Mutex<Option<ConnectedExchange>>,
    revoked: std::sync::atomic::AtomicBool,
    live_lease: Mutex<Option<Weak<McpProviderExchange>>>,
}
impl CommandPort {
    pub(crate) fn surface_version(&self) -> &CapabilitySurfaceVersion {
        &self.version
    }
    fn revoke_now(&self) {
        self.revoked
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(lease) = self.live_lease.lock()
            && let Some(lease) = lease.as_ref().and_then(Weak::upgrade)
        {
            lease.revoke();
        }
    }
    async fn prepare(&self) -> Result<(), AgentLoopHostError> {
        if self.revoked.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(unavailable());
        }
        if self.definitions.is_empty() {
            return Ok(());
        }
        let mut connected = self.connection.lock().await;
        if connected.is_some() {
            return Ok(());
        }
        let lease = Arc::new(
            self.bridge
                .connect(self.context.run_id.as_uuid())
                .await
                .map_err(|_| unavailable())?,
        );
        *self.live_lease.lock().map_err(|_| unavailable())? = Some(Arc::downgrade(&lease));
        if self.revoked.load(std::sync::atomic::Ordering::SeqCst) {
            lease.revoke();
            return Err(unavailable());
        }
        if !lease.matches_advertisement(&self.snapshot) {
            lease.disconnect().await.map_err(|_| unavailable())?;
            return Err(unavailable());
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| unavailable())?;
        let connection = ConnectedExchange {
            lease,
            client,
            endpoint: self.endpoint.clone(),
        };
        let result=connection.post(json!({"jsonrpc":"2.0","id":"initialize","method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"brassclaw-kohai","version":env!("CARGO_PKG_VERSION")}}}),false).await?;
        if result["protocolVersion"] != "2025-06-18" {
            return Err(unavailable());
        }
        connection
            .post(
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                true,
            )
            .await?;
        let listed = connection
            .post(
                json!({"jsonrpc":"2.0","id":"listing","method":"tools/list"}),
                true,
            )
            .await?;
        if listed != self.snapshot.tools_list() {
            return Err(unavailable());
        }
        *connected = Some(connection);
        Ok(())
    }
    async fn close_after_answer(&self) -> Result<(), AgentLoopHostError> {
        let connection = self.connection.lock().await.take();
        if let Some(connection) = connection {
            connection
                .lease
                .disconnect()
                .await
                .map_err(|_| unavailable())?;
        }
        Ok(())
    }
    async fn finish(&self) -> Result<(), AgentLoopHostError> {
        self.revoke_now();
        let connection = self.connection.lock().await.take();
        if let Some(connection) = connection {
            connection
                .lease
                .disconnect()
                .await
                .map_err(|_| unavailable())?;
        }
        Ok(())
    }
    fn definition(&self, name: &str) -> Result<&ProviderToolDefinition, AgentLoopHostError> {
        self.definitions
            .iter()
            .find(|definition| definition.name == name)
            .ok_or_else(invalid)
    }
    fn validate(&self, call: &ProviderToolCall) -> Result<(), AgentLoopHostError> {
        if self.revoked.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(unavailable());
        }
        self.definition(&call.name)?;
        let object = call.arguments.as_object().ok_or_else(invalid)?;
        if object.len() != 1
            || object
                .get("command")
                .and_then(Value::as_str)
                .is_none_or(|command| command.trim().is_empty() || command.len() > 64 * 1024)
            || call.id.is_empty()
            || call.turn_id.as_ref().is_none_or(String::is_empty)
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[async_trait]
impl LoopCapabilityPort for CommandPort {
    fn tool_definitions(&self) -> Result<Vec<ProviderToolDefinition>, AgentLoopHostError> {
        Ok(self.definitions.clone())
    }
    fn validate_provider_tool_call(
        &self,
        call: &ProviderToolCall,
    ) -> Result<(), AgentLoopHostError> {
        self.validate(call)
    }
    async fn register_provider_tool_call(
        &self,
        call: ProviderToolCall,
    ) -> Result<CapabilityCallCandidate, AgentLoopHostError> {
        self.validate(&call)?;
        let id = self.definition(&call.name)?.capability_id.clone();
        let key=format!("input:mcp-{}",hex::encode(Sha256::digest(serde_json::to_vec(&json!({"run":self.context.run_id,"provider":call.provider_id,"model":call.provider_model_id,"turn":call.turn_id,"id":call.id})).map_err(|_| invalid())?)));
        let input_ref = CapabilityInputRef::new(key.clone()).map_err(|_| invalid())?;
        let replay = ProviderToolCallReplay {
            provider_id: call.provider_id.clone(),
            provider_model_id: call.provider_model_id.clone(),
            provider_turn_id: call.turn_id.clone().ok_or_else(invalid)?,
            provider_call_id: call.id.clone(),
            provider_tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
            response_reasoning: call.response_reasoning.clone(),
            reasoning: call.reasoning.clone(),
            signature: call.signature.clone(),
        };
        let mut inputs = self.inputs.lock().map_err(|_| unavailable())?;
        if let Some(original) = inputs.get(&key)
            && original != &call
        {
            return Err(invalid());
        }
        inputs.insert(key, call);
        Ok(CapabilityCallCandidate {
            surface_version: self.version.clone(),
            capability_id: id.clone(),
            input_ref,
            effective_capability_ids: vec![id],
            provider_replay: Some(replay),
        })
    }
    async fn visible_capabilities(
        &self,
        _request: VisibleCapabilityRequest,
    ) -> Result<VisibleCapabilitySurface, AgentLoopHostError> {
        Ok(VisibleCapabilitySurface {
            version: self.version.clone(),
            descriptors: self
                .definitions
                .iter()
                .map(|definition| CapabilityDescriptorView {
                    capability_id: definition.capability_id.clone(),
                    provider: None,
                    runtime: RuntimeKind::Mcp,
                    safe_name: definition.name.clone(),
                    safe_description: definition.description.clone(),
                    concurrency_hint: ConcurrencyHint::Exclusive,
                    parameters_schema: definition.parameters.clone(),
                })
                .collect(),
        })
    }
    async fn invoke_capability(
        &self,
        request: CapabilityInvocation,
    ) -> Result<CapabilityOutcome, AgentLoopHostError> {
        if self.revoked.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(unavailable());
        }
        if request.surface_version != self.version {
            return Err(invalid());
        }
        let call = self
            .inputs
            .lock()
            .map_err(|_| unavailable())?
            .get(request.input_ref.as_str())
            .cloned()
            .ok_or_else(invalid)?;
        if self.definition(&call.name)?.capability_id != request.capability_id {
            return Err(invalid());
        }
        let mut results = self.results.lock().await;
        if let Some(result) = results.get(request.input_ref.as_str()) {
            return Ok(CapabilityOutcome::Completed(result.clone()));
        }
        let connection = self
            .connection
            .lock()
            .await
            .clone()
            .ok_or_else(unavailable)?;
        let response=connection.post(json!({"jsonrpc":"2.0","id":request.input_ref.as_str(),"method":"tools/call","params":{"name":call.name,"arguments":call.arguments}}),true).await?;
        if self.revoked.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(unavailable());
        }
        let result_ref = self
            .writer
            .write_capability_result(CapabilityResultWrite {
                run_context: &self.context,
                input_ref: &request.input_ref,
                invocation_id: InvocationId::new(),
                capability_id: &request.capability_id,
                output: response,
                display_preview: None,
            })
            .await?;
        let result = CapabilityResultMessage {
            result_ref,
            safe_summary: "MCP ordinary chat completed".into(),
            progress: CapabilityProgress::MadeProgress,
            terminate_hint: false,
        };
        results.insert(request.input_ref.as_str().to_owned(), result.clone());
        Ok(CapabilityOutcome::Completed(result))
    }
    async fn invoke_capability_batch(
        &self,
        request: CapabilityBatchInvocation,
    ) -> Result<CapabilityBatchOutcome, AgentLoopHostError> {
        let mut outcomes = Vec::new();
        for invocation in request.invocations {
            outcomes.push(self.invoke_capability(invocation).await?);
        }
        Ok(CapabilityBatchOutcome {
            outcomes,
            stopped_on_suspension: false,
        })
    }
}

pub(crate) struct McpProviderModelGateway {
    pub(crate) inner: Arc<dyn HostManagedModelGateway>,
    pub(crate) binding: Arc<McpProviderBinding>,
}
struct ExchangeGuard {
    binding: Arc<McpProviderBinding>,
    port: Arc<CommandPort>,
    retained: bool,
}
impl Drop for ExchangeGuard {
    fn drop(&mut self) {
        if !self.retained {
            self.port.revoke_now();
            let binding = self.binding.clone();
            let port = self.port.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    binding.finish_port(&port).await;
                });
            }
        }
    }
}
#[async_trait]
impl HostManagedModelGateway for McpProviderModelGateway {
    fn supports_tool_exchange(&self) -> bool {
        self.inner.supports_tool_exchange()
    }

    async fn stream_model(
        &self,
        request: HostManagedModelRequest,
    ) -> Result<HostManagedModelResponse, HostManagedModelError> {
        self.inner.stream_model(request).await
    }
    async fn stream_model_with_capabilities(
        &self,
        request: HostManagedModelRequest,
        capabilities: Arc<dyn LoopCapabilityPort>,
    ) -> Result<HostManagedModelResponse, HostManagedModelError> {
        let run_id = request.run_id;
        let port = self.binding.port(run_id).map_err(|_| {
            HostManagedModelError::safe(
                HostManagedModelErrorKind::Unavailable,
                "MCP exchange is unavailable",
            )
        })?;
        let Some(port) = port else {
            return self
                .inner
                .stream_model_with_capabilities(request, capabilities)
                .await;
        };
        if request.surface_version.as_ref() != Some(&port.version) {
            return Err(HostManagedModelError::safe(
                HostManagedModelErrorKind::Unavailable,
                "MCP exchange belongs to a different task selection",
            ));
        }
        if !port.definitions.is_empty() && !self.inner.supports_tool_exchange() {
            self.binding.finish_port(&port).await;
            return Err(HostManagedModelError::safe(
                HostManagedModelErrorKind::Unavailable,
                "selected provider gateway does not support MCP command advertisement",
            ));
        }
        let mut guard = ExchangeGuard {
            binding: self.binding.clone(),
            port: port.clone(),
            retained: false,
        };
        port.prepare().await.map_err(|_| {
            HostManagedModelError::safe(
                HostManagedModelErrorKind::Unavailable,
                "MCP exchange initialization failed",
            )
        })?;
        let result = self
            .inner
            .stream_model_with_capabilities(request, capabilities)
            .await;
        match result.as_ref().map(|response| &response.output) {
            Ok(ParentLoopOutput::CapabilityCalls(_)) => {
                guard.retained = true;
            }
            Ok(ParentLoopOutput::AssistantReply(_)) => {
                port.close_after_answer().await.map_err(|_| {
                    HostManagedModelError::safe(
                        HostManagedModelErrorKind::Unavailable,
                        "MCP exchange disconnect failed",
                    )
                })?;
                guard.retained = true;
            }
            Err(_) => {
                self.binding.finish_port(&port).await;
                guard.retained = true;
            }
        }
        result
    }
}
