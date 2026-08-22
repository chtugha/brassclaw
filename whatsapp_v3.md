# WhatsApp Channel — v3 Integration Plan

> **Status:** Draft — implementation ready. No code is changed by this document.
> **Scope:** Integrate WhatsApp Business Cloud API as a first-class `ProductAdapter`
> in the Reborn v3 stack, wired as a chat surface identical to Telegram v2. The agent
> account you have created maps to one Meta WABA phone number. Every user that DMs that
> number reaches the agent. Replies are delivered back to the sender, exactly as a chat.
>
> **Next migration:** V062 (V061 = `reborn_components_registry`, already applied).
>
> **Companion document:** `saved_plan_to_v3.md` — the recipe/v3 finalisation plan.
> WhatsApp integration is additive on top of that plan. Neither plan modifies the
> other's DB tables. They are safe to execute in parallel or sequentially.

---

## Working Rules

Identical to `saved_plan_to_v3.md`:

- Implement **one phase at a time**. Never batch.
- After each phase is fully resolved: mark it **[DONE]**, commit, push to
  `origin/main`, then continue.
- Address every finding encountered — never suppress or silence.
- No `.unwrap()` / `.expect()` in production code (test-level and
  compile-time-constant safety comments are the only exceptions).
- Keep clippy clean: `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`.
- Use `debug!` for internal diagnostics; never `info!` or `warn!` from
  background tasks (corrupts TUI).
- Run the targeted validation command at the end of each phase before marking
  it done.
- Never use `git stash` — commit everything.

---

## 0. Architecture Vision

### 0.1 What "WhatsApp as a chat" means

The goal is that the WebUI shows a WhatsApp conversation alongside the normal
web-chat threads. From the inside, the BrassClaw pipeline sees no difference:

```
User sends WhatsApp DM to agent phone number
  → Meta delivers POST to /webhooks/whatsapp
    → Host verifies HMAC-SHA256 (X-Hub-Signature-256)
      → WhatsAppV2Adapter::parse_inbound()
        → ProductWorkflow::submit_inbound()
          → ConversationBindingService → SessionThread (created/reused)
            → TurnCoordinator → agent loop runs
              → WhatsAppV2Adapter::render_outbound()
                → POST graph.facebook.com/.../messages (Bearer token)
                  → WhatsApp message delivered to user
```

The `SessionThread` created for a WhatsApp conversation is identical to one
created for a WebUI conversation. Its `thread_id` is visible in the WebUI.
The user sees it in the thread list. The Settings page shows the WhatsApp
installation status and lets the operator configure credentials.

### 0.2 Positioning in the v3 layer stack

```
brassclaw_whatsapp_v2_adapter   ← NEW — mirrors brassclaw_telegram_v2_adapter
    │ implements ProductAdapter trait (brassclaw_product_adapters)
    │
brassclaw_reborn_composition    ← MODIFIED — wires adapter, routes, secrets
    │ src/whatsapp.rs  (new)
    │ src/webui_serve.rs  (modified — adds webhook routes)
    │
brassclaw_webui_v2              ← MODIFIED — adds Settings > Channels panel routes
    │ src/handlers/whatsapp_settings.rs  (new)
    │ src/descriptors.rs  (new route constants)
    │ src/router.rs  (new routes mounted)
    │
crates/brassclaw_pg/migrations  ← MODIFIED — V062 adds whatsapp_installations table
    │
brassclaw_reborn_traces         ← MODIFIED — TraceChannel::WhatsApp variant
    │
brassclaw_product_workflow      ← MODIFIED — StaticConnectableChannelsProductFacade
```

### 0.3 Crate boundaries that must not be crossed

- `brassclaw_whatsapp_v2_adapter` must NOT depend on:
  `brassclaw_dispatcher`, `brassclaw_capabilities`, `brassclaw_host_runtime`,
  `brassclaw_network`, `brassclaw_secrets`, `brassclaw_filesystem`,
  `brassclaw_turns::runner`.
- The adapter may ONLY perform network I/O via the `ProtocolHttpEgress` trait.
- The adapter may NOT construct `TrustedInboundTurnRequest` or call any
  trusted trigger submitter — those paths are sealed inside
  `brassclaw_conversations`.
- Product adapters must never mint `ProtocolAuthEvidence::Verified` — only the
  host verifier (called before `parse_inbound`) does that.

### 0.4 Credential model

Three secrets, all operator-tier (never persisted in config files, only in the
encrypted secret store):

| Secret name | What it is | Resolved by |
|---|---|---|
| `whatsapp_access_token` | Meta system-user bearer token | Host egress (at send time) via `EgressCredentialHandle` |
| `whatsapp_app_secret` | HMAC-SHA256 key for `X-Hub-Signature-256` | Host HMAC verifier (never enters adapter) |
| `whatsapp_verify_token` | Shared secret for META GET challenge | Host GET handler (never enters adapter) |

### 0.5 Reply-target encoding

`ReplyTargetBindingRef` uses the scheme: **`wa:<e164_phone_number>`**

Example: `wa:+4915112345678`

The phone number is the WhatsApp sender's `wa_id` field, which Meta delivers as
an E.164 number without `+` prefix (e.g. `4915112345678`). We store it with
a `+` prefix for canonical E.164 form. Rendering strips it back to digits-only
for the Meta API `"to"` field.

### 0.6 Migration plan (V062)

One new table: `reborn_whatsapp_installations`. This is the persistence layer
for the operator-configured WhatsApp installation. The secrets themselves live in
the encrypted `brassclaw_secrets` store — the installation row only records
metadata (phone number ID, display name, enabled flag, credential handle names).

---

## 1. Phase WA-A — New crate `brassclaw_whatsapp_v2_adapter`

**Goal:** A complete, tested, clippy-clean `ProductAdapter` implementation
for WhatsApp Business Cloud API. Mirrors `brassclaw_telegram_v2_adapter` exactly.
No composition wiring yet — the crate just compiles and passes its own tests.

### 1.1 Create the crate skeleton

**File: `crates/brassclaw_whatsapp_v2_adapter/Cargo.toml`**

```toml
[package]
name = "brassclaw_whatsapp_v2_adapter"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
brassclaw_product_adapters = { path = "../brassclaw_product_adapters" }
brassclaw_turns = { path = "../brassclaw_turns" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
```

**File: `crates/brassclaw_whatsapp_v2_adapter/src/lib.rs`**

```rust
//! WhatsApp Business Cloud API v2 ProductAdapter.
//!
//! Mirrors brassclaw_telegram_v2_adapter. The adapter is pure translation:
//! parse inbound Meta webhook payloads → ProductInboundEnvelope, and render
//! outbound envelopes → Meta Cloud API HTTP requests. No business logic here.

#![forbid(unsafe_code)]

mod adapter;
mod payload;
mod render;

pub use adapter::{WhatsAppV2Adapter, WhatsAppV2AdapterConfig, whatsapp_default_capabilities};
pub use payload::{WHATSAPP_API_HOST, WHATSAPP_USER_ACTOR_KIND, parse_whatsapp_webhook};
pub use render::{WhatsAppRenderError, render_final_reply, render_progress_reaction};
```

Add `brassclaw_whatsapp_v2_adapter` to the workspace `Cargo.toml` members list.

### 1.2 Config and adapter struct

**File: `crates/brassclaw_whatsapp_v2_adapter/src/adapter.rs`**

```rust
use async_trait::async_trait;
use brassclaw_product_adapters::redaction::RedactedString;
use brassclaw_product_adapters::{
    AdapterInstallationId, AuthRequirement, DeclaredEgressHost, DeclaredEgressTarget,
    DeliveryStatus, EgressCredentialHandle, OutboundDeliverySink, ParsedProductInbound,
    ProductAdapter, ProductAdapterCapabilities, ProductAdapterError, ProductAdapterId,
    ProductCapabilityFlag, ProductOutboundEnvelope, ProductOutboundPayload, ProductRenderOutcome,
    ProductSurfaceKind, ProtocolAuthEvidence, ProtocolHttpEgress, ProtocolHttpEgressError,
};
use brassclaw_turns::{ReplyTargetBindingRef, TurnRunId};

use crate::payload::{WHATSAPP_API_HOST, parse_whatsapp_webhook};
use crate::render::{render_final_reply, render_progress_reaction};

pub const WHATSAPP_ACTOR_KIND: &str = "whatsapp_user";

/// Configuration for a single WhatsApp Business Cloud API installation.
/// One installation = one Meta WABA phone number.
#[derive(Debug, Clone)]
pub struct WhatsAppV2AdapterConfig {
    pub adapter_id: ProductAdapterId,
    pub installation_id: AdapterInstallationId,
    /// Meta phone number ID (e.g. "123456789012345"). Used as the egress
    /// path segment: /v19.0/{phone_number_id}/messages
    pub phone_number_id: String,
    /// Credential handle resolved by the host to the bearer token at egress time.
    pub egress_credential_handle: EgressCredentialHandle,
    /// Auth requirement. WhatsApp uses HMAC-SHA256 over the raw body with the
    /// app secret, sent in X-Hub-Signature-256: sha256=<hex>.
    /// Map to AuthRequirement::RequestSignature.
    pub auth_requirement: AuthRequirement,
    /// When true, the adapter advertises ExternalProgressPush and renders a
    /// "read" reaction on outbound Progress envelopes. Default: false.
    pub progress_push_enabled: bool,
    /// Name of the secret holding the webhook verify token for GET challenge
    /// verification. Per-installation so multi-installation setups can use
    /// distinct secrets. Defaults to `"whatsapp_verify_token"`.
    pub verify_token_secret_name: String,
}

pub struct WhatsAppV2Adapter {
    config: WhatsAppV2AdapterConfig,
    capabilities: ProductAdapterCapabilities,
    declared_egress: Vec<DeclaredEgressTarget>,
}

impl WhatsAppV2Adapter {
    pub fn new(config: WhatsAppV2AdapterConfig) -> Self {
        let mut capabilities = whatsapp_default_capabilities();
        if config.progress_push_enabled {
            capabilities = capabilities.with(ProductCapabilityFlag::ExternalProgressPush);
        }
        let declared_egress = vec![DeclaredEgressTarget::new(
            // safety: WHATSAPP_API_HOST is a compile-time const satisfying the validator
            DeclaredEgressHost::new(WHATSAPP_API_HOST).expect("static host valid"),
            Some(config.egress_credential_handle.clone()),
        )];
        Self { config, capabilities, declared_egress }
    }

    pub fn config(&self) -> &WhatsAppV2AdapterConfig {
        &self.config
    }
}

impl WhatsAppV2AdapterConfig {
    /// Name of the secret holding the webhook verify token (for the GET challenge handler).
    pub fn verify_token_secret_name(&self) -> &str {
        &self.verify_token_secret_name
    }
}

pub fn whatsapp_default_capabilities() -> ProductAdapterCapabilities {
    ProductAdapterCapabilities::external_channel_default()
}

#[async_trait]
impl ProductAdapter for WhatsAppV2Adapter {
    fn adapter_id(&self) -> &ProductAdapterId { &self.config.adapter_id }
    fn installation_id(&self) -> &AdapterInstallationId { &self.config.installation_id }
    fn surface_kind(&self) -> ProductSurfaceKind { ProductSurfaceKind::ExternalChannel }
    fn capabilities(&self) -> &ProductAdapterCapabilities { &self.capabilities }
    fn auth_requirement(&self) -> &AuthRequirement { &self.config.auth_requirement }
    fn declared_egress(&self) -> &[DeclaredEgressTarget] { &self.declared_egress }

    fn parse_inbound(
        &self,
        raw_payload: &[u8],
        auth_evidence: &ProtocolAuthEvidence,
    ) -> Result<ParsedProductInbound, ProductAdapterError> {
        parse_whatsapp_webhook(raw_payload, auth_evidence, &self.config.installation_id)
            .map_err(|err| match err {
                crate::payload::PayloadParseError::UnauthenticatedPayload =>
                    ProductAdapterError::Authentication(
                        brassclaw_product_adapters::ProtocolAuthFailure::Missing,
                    ),
                crate::payload::PayloadParseError::InvalidJson { reason } =>
                    ProductAdapterError::MalformedInboundPayload {
                        reason: RedactedString::new(reason),
                    },
                crate::payload::PayloadParseError::MissingMessageId =>
                    ProductAdapterError::MalformedInboundPayload {
                        reason: RedactedString::new("whatsapp message missing wamid"),
                    },
                crate::payload::PayloadParseError::InvalidExternalRef { kind, reason } =>
                    ProductAdapterError::InvalidIdentifier { kind, reason },
            })
    }

    async fn render_outbound(
        &self,
        envelope: ProductOutboundEnvelope,
        egress: &dyn ProtocolHttpEgress,
        delivery_sink: &dyn OutboundDeliverySink,
    ) -> Result<ProductRenderOutcome, ProductAdapterError> {
        // Fail closed on installation mismatch — same invariant as Telegram.
        if envelope.adapter_id != self.config.adapter_id {
            return Err(ProductAdapterError::InvalidIdentifier {
                kind: "envelope.adapter_id",
                reason: format!(
                    "envelope adapter_id `{}` does not match this adapter `{}`",
                    envelope.adapter_id.as_str(),
                    self.config.adapter_id.as_str(),
                ),
            });
        }
        if envelope.installation_id != self.config.installation_id {
            return Err(ProductAdapterError::InvalidIdentifier {
                kind: "envelope.installation_id",
                reason: format!(
                    "envelope installation_id `{}` does not match installation `{}`",
                    envelope.installation_id.as_str(),
                    self.config.installation_id.as_str(),
                ),
            });
        }

        // Extract all fields before the move-consuming match on envelope.payload.
        let attempt_id = envelope.delivery_attempt_id;
        let target_binding = envelope.target.reply_target_binding_ref.clone();
        let run_id: Option<TurnRunId> = match &envelope.payload {
            ProductOutboundPayload::FinalReply(v) => Some(v.turn_run_id),
            ProductOutboundPayload::Progress(v) => Some(v.turn_run_id),
            _ => None,
        };
        // Clone the reply-target ref before envelope.payload is moved so the
        // render helpers below can borrow it from the stack-owned clone.
        let reply_target = target_binding.clone();

        let request = match envelope.payload {
            ProductOutboundPayload::FinalReply(view) => {
                match render_final_reply(
                    &reply_target,
                    &view,
                    self.config.egress_credential_handle.clone(),
                    &self.config.phone_number_id,
                ) {
                    Ok(req) => req,
                    Err(render_err) => {
                        record_status(delivery_sink, DeliveryStatus::FailedPermanent {
                            attempt_id, target: target_binding.clone(), run_id,
                            reason: RedactedString::new(render_err.to_string()),
                        }).await;
                        return Err(map_render_error(render_err));
                    }
                }
            }
            ProductOutboundPayload::Progress(view) => {
                if !self.capabilities.contains(ProductCapabilityFlag::ExternalProgressPush) {
                    record_status(delivery_sink, DeliveryStatus::Deferred {
                        attempt_id, target: target_binding.clone(), run_id,
                        reason: RedactedString::new("progress not advertised on this installation"),
                    }).await;
                    return Ok(ProductRenderOutcome::Deferred);
                }
                match render_progress_reaction(
                    &reply_target,
                    &view,
                    self.config.egress_credential_handle.clone(),
                    &self.config.phone_number_id,
                ) {
                    Ok(Some(req)) => req,
                    Ok(None) => {
                        record_status(delivery_sink, DeliveryStatus::Deferred {
                            attempt_id, target: target_binding.clone(), run_id,
                            reason: RedactedString::new("progress kind did not map to a WA action"),
                        }).await;
                        return Ok(ProductRenderOutcome::Deferred);
                    }
                    Err(render_err) => {
                        record_status(delivery_sink, DeliveryStatus::FailedPermanent {
                            attempt_id, target: target_binding.clone(), run_id,
                            reason: RedactedString::new(render_err.to_string()),
                        }).await;
                        return Err(map_render_error(render_err));
                    }
                }
            }
            ProductOutboundPayload::GatePrompt(_) | ProductOutboundPayload::AuthPrompt(_) => {
                // Deferred — gate/auth prompts via WhatsApp text messages is a follow-up.
                record_status(delivery_sink, DeliveryStatus::Deferred {
                    attempt_id, target: target_binding.clone(), run_id: None,
                    reason: RedactedString::new("gate/auth prompts deferred on WhatsApp"),
                }).await;
                return Ok(ProductRenderOutcome::Deferred);
            }
            ProductOutboundPayload::CapabilityActivity(_)
            | ProductOutboundPayload::CapabilityDisplayPreview(_)
            | ProductOutboundPayload::ProjectionSnapshot { .. }
            | ProductOutboundPayload::ProjectionUpdate { .. }
            | ProductOutboundPayload::KeepAlive => {
                record_status(delivery_sink, DeliveryStatus::Deferred {
                    attempt_id, target: target_binding.clone(), run_id: None,
                    reason: RedactedString::new("whatsapp surface does not consume projection envelopes"),
                }).await;
                return Ok(ProductRenderOutcome::Deferred);
            }
        };

        let response = match egress.send(request).await {
            Ok(resp) => resp,
            Err(egress_err) => {
                record_status(delivery_sink,
                    egress_err_to_delivery_status(&egress_err, attempt_id, target_binding.clone(), run_id),
                ).await;
                return Err(map_egress_error(egress_err));
            }
        };

        if !(200..300).contains(&response.status()) {
            let reason = RedactedString::new(format!(
                "whatsapp api returned status {}", response.status()
            ));
            if response.status() >= 500 || response.status() == 429 {
                record_status(delivery_sink, DeliveryStatus::FailedRetryable {
                    attempt_id, target: target_binding.clone(), run_id, reason: reason.clone(),
                }).await;
                return Err(ProductAdapterError::WorkflowTransient { reason });
            }
            if response.status() == 401 || response.status() == 403 {
                record_status(delivery_sink, DeliveryStatus::FailedUnauthorized {
                    attempt_id, target: target_binding.clone(), run_id, reason: reason.clone(),
                }).await;
            } else {
                record_status(delivery_sink, DeliveryStatus::FailedPermanent {
                    attempt_id, target: target_binding.clone(), run_id, reason: reason.clone(),
                }).await;
            }
            return Err(ProductAdapterError::EgressDenied { reason });
        }

        record_status(delivery_sink, DeliveryStatus::Delivered {
            attempt_id, target: target_binding, run_id,
        }).await;
        Ok(ProductRenderOutcome::DeliveryRecorded)
    }
}

async fn record_status(sink: &dyn OutboundDeliverySink, status: DeliveryStatus) {
    sink.record(status).await;
}

fn egress_err_to_delivery_status(
    err: &ProtocolHttpEgressError,
    attempt_id: brassclaw_product_adapters::DeliveryAttemptId,
    target: ReplyTargetBindingRef,
    run_id: Option<TurnRunId>,
) -> DeliveryStatus {
    let reason = RedactedString::new(err.to_string());
    match err {
        ProtocolHttpEgressError::Timeout
        | ProtocolHttpEgressError::Network(_)
        | ProtocolHttpEgressError::LeakDetected => DeliveryStatus::FailedRetryable {
            attempt_id, target, run_id, reason,
        },
        ProtocolHttpEgressError::UnknownCredentialHandle { .. }
        | ProtocolHttpEgressError::UnauthorizedCredentialHandle { .. } =>
            DeliveryStatus::FailedUnauthorized { attempt_id, target, run_id, reason },
        ProtocolHttpEgressError::UndeclaredHost { .. }
        | ProtocolHttpEgressError::PolicyDenied { .. } =>
            DeliveryStatus::FailedPermanent { attempt_id, target, run_id, reason },
    }
}

fn map_render_error(err: crate::render::WhatsAppRenderError) -> ProductAdapterError {
    match err {
        crate::render::WhatsAppRenderError::InvalidReplyTarget { .. } =>
            ProductAdapterError::InvalidIdentifier {
                kind: "reply_target",
                reason: err.to_string(),
            },
    }
}

fn map_egress_error(err: ProtocolHttpEgressError) -> ProductAdapterError {
    let reason = RedactedString::new(err.to_string());
    match err {
        ProtocolHttpEgressError::Timeout
        | ProtocolHttpEgressError::Network(_)
        | ProtocolHttpEgressError::LeakDetected =>
            ProductAdapterError::WorkflowTransient { reason },
        ProtocolHttpEgressError::UndeclaredHost { .. }
        | ProtocolHttpEgressError::UnknownCredentialHandle { .. }
        | ProtocolHttpEgressError::UnauthorizedCredentialHandle { .. }
        | ProtocolHttpEgressError::PolicyDenied { .. } =>
            ProductAdapterError::EgressDenied { reason },
    }
}
```

### 1.3 Payload parsing

**File: `crates/brassclaw_whatsapp_v2_adapter/src/payload.rs`**

The Meta Cloud API delivers payloads with this shape (v19.0+):

```json
{
  "object": "whatsapp_business_account",
  "entry": [{
    "id": "<WABA_ID>",
    "changes": [{
      "value": {
        "messaging_product": "whatsapp",
        "metadata": { "phone_number_id": "...", "display_phone_number": "..." },
        "contacts": [{ "profile": { "name": "Alice" }, "wa_id": "4915112345678" }],
        "messages": [{
          "id": "wamid.xxx",
          "from": "4915112345678",
          "timestamp": "1681234567",
          "type": "text",
          "text": { "body": "Hello agent" }
        }],
        "statuses": []
      },
      "field": "messages"
    }]
  }]
}
```

Key mapping:

| Meta field | BrassClaw type | Notes |
|---|---|---|
| `messages[0].id` (wamid) | `ExternalEventId` | Dedupe key per installation |
| `messages[0].from` (wa_id, digits only) | `ExternalActorRef` kind=`whatsapp_user` | E.164 without `+` |
| `contacts[0].profile.name` | `ExternalActorRef.display_name` | Optional |
| `metadata.phone_number_id` | `ExternalConversationRef.id` | "The chat" is the 1:1 DM channel |
| `messages[0].text.body` | `UserMessagePayload.text` | Plain text only (initial slice) |
| `statuses[]` entries | `ProductInboundPayload::NoOp` | Delivery receipts — never reach agent |
| Non-text `message.type` | `ProductInboundPayload::NoOp` | image/audio/doc — deferred to Phase WA-H |

```rust
//! WhatsApp Business Cloud API webhook payload normalization.
//!
//! Inputs are raw bytes from a verified POST webhook request.
//! auth_evidence MUST be Verified before this function is called —
//! the host HMAC verifier owns that check.

pub const WHATSAPP_API_HOST: &str = "graph.facebook.com";
pub const WHATSAPP_USER_ACTOR_KIND: &str = "whatsapp_user";

use brassclaw_product_adapters::{
    AdapterInstallationId, ExternalActorRef, ExternalConversationRef, ExternalEventId,
    ParsedProductInbound, ProductInboundPayload, ProductTriggerReason, ProtocolAuthEvidence,
    UserMessagePayload,
};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PayloadParseError {
    #[error("invalid WhatsApp webhook JSON: {reason}")]
    InvalidJson { reason: String },
    #[error("WhatsApp message missing wamid")]
    MissingMessageId,
    #[error("invalid external reference: {kind}: {reason}")]
    InvalidExternalRef { kind: &'static str, reason: String },
    #[error("auth evidence is not Verified — host MUST verify HMAC before calling parse_whatsapp_webhook")]
    UnauthenticatedPayload,
}

/// Top-level Meta webhook envelope
#[derive(Debug, Deserialize)]
struct MetaWebhookBody {
    entry: Vec<MetaEntry>,
}

#[derive(Debug, Deserialize)]
struct MetaEntry {
    changes: Vec<MetaChange>,
}

#[derive(Debug, Deserialize)]
struct MetaChange {
    value: MetaChangeValue,
}

#[derive(Debug, Deserialize)]
struct MetaChangeValue {
    metadata: MetaMetadata,
    #[serde(default)]
    contacts: Vec<MetaContact>,
    #[serde(default)]
    messages: Vec<MetaMessage>,
    #[serde(default)]
    statuses: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct MetaMetadata {
    phone_number_id: String,
}

#[derive(Debug, Deserialize)]
struct MetaContact {
    profile: MetaProfile,
    wa_id: String,
}

#[derive(Debug, Deserialize)]
struct MetaProfile {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MetaMessage {
    id: String,   // wamid
    from: String, // wa_id (E.164 digits without +)
    #[serde(rename = "type")]
    kind: String,
    text: Option<MetaTextBody>,
}

#[derive(Debug, Deserialize)]
struct MetaTextBody {
    body: String,
}

pub fn parse_whatsapp_webhook(
    raw_payload: &[u8],
    auth_evidence: &ProtocolAuthEvidence,
    installation_id: &AdapterInstallationId,
) -> Result<ParsedProductInbound, PayloadParseError> {
    if !auth_evidence.is_verified() {
        return Err(PayloadParseError::UnauthenticatedPayload);
    }

    let body: MetaWebhookBody =
        serde_json::from_slice(raw_payload).map_err(|e| PayloadParseError::InvalidJson {
            reason: e.to_string(),
        })?;

    // Walk entry[0].changes[0].value — we take the first actionable message.
    // Delivery-status-only payloads (statuses[] non-empty, messages[] empty)
    // return a NoOp so the webhook acks 200 without polluting the agent.
    for entry in &body.entry {
        for change in &entry.changes {
            let value = &change.value;

            // Statuses-only: return NoOp immediately
            if !value.statuses.is_empty() && value.messages.is_empty() {
                return build_noop(installation_id, &value.metadata.phone_number_id);
            }

            for message in &value.messages {
                // Non-text types (image, audio, document, etc.) are NoOp in initial slice.
                if message.kind != "text" {
                    return build_noop(installation_id, &value.metadata.phone_number_id);
                }

                let wamid = &message.id;
                if wamid.is_empty() {
                    return Err(PayloadParseError::MissingMessageId);
                }

                // External event id = "<installation_id>:<wamid>" for global uniqueness
                let event_id_raw = format!("{}:{}", installation_id.as_str(), wamid);
                let event_id = ExternalEventId::new(event_id_raw)
                    .map_err(|e| PayloadParseError::InvalidExternalRef {
                        kind: "external_event_id",
                        reason: e.to_string(),
                    })?;

                // Actor: the sender's wa_id (digits only E.164)
                let display_name = value.contacts.iter()
                    .find(|c| c.wa_id == message.from)
                    .map(|c| c.profile.name.as_str());
                // Store with + prefix for canonical E.164
                let actor_id = format!("+{}", message.from);
                let actor_ref = ExternalActorRef::new(
                    WHATSAPP_USER_ACTOR_KIND,
                    actor_id,
                    display_name,
                ).map_err(|e| PayloadParseError::InvalidExternalRef {
                    kind: "external_actor_ref",
                    reason: e.to_string(),
                })?;

                // Conversation: keyed on phone_number_id (the agent's DM "chat")
                let conversation_ref = ExternalConversationRef::new(
                    None,
                    value.metadata.phone_number_id.clone(),
                    None::<&str>,
                    None::<&str>,
                ).map_err(|e| PayloadParseError::InvalidExternalRef {
                    kind: "external_conversation_ref",
                    reason: e.to_string(),
                })?;

                let text = message.text.as_ref()
                    .map(|t| t.body.as_str())
                    .unwrap_or("")
                    .to_string();

                let payload = ProductInboundPayload::UserMessage(
                    UserMessagePayload::new(text, vec![], ProductTriggerReason::DirectChat)
                        .map_err(|e| PayloadParseError::InvalidExternalRef {
                            kind: "user_message_payload",
                            reason: e.to_string(),
                        })?,
                );

                return ParsedProductInbound::new(event_id, actor_ref, conversation_ref, payload)
                    .map_err(|e| PayloadParseError::InvalidExternalRef {
                        kind: "parsed_product_inbound",
                        reason: e.to_string(),
                    });
            }
        }
    }

    // No actionable message found (empty entry, empty changes, etc.)
    build_noop(installation_id, "noop")
}

fn build_noop(
    installation_id: &AdapterInstallationId,
    conversation_id: &str,
) -> Result<ParsedProductInbound, PayloadParseError> {
    let event_id = ExternalEventId::new(format!("{}:noop:{}", installation_id.as_str(), uuid::Uuid::new_v4()))
        .map_err(|e| PayloadParseError::InvalidExternalRef {
            kind: "external_event_id", reason: e.to_string(),
        })?;
    let actor = ExternalActorRef::new("whatsapp_system", "noop", None::<&str>)
        .map_err(|e| PayloadParseError::InvalidExternalRef {
            kind: "external_actor_ref", reason: e.to_string(),
        })?;
    let conv = ExternalConversationRef::new(None, conversation_id, None::<&str>, None::<&str>)
        .map_err(|e| PayloadParseError::InvalidExternalRef {
            kind: "external_conversation_ref", reason: e.to_string(),
        })?;
    ParsedProductInbound::new(event_id, actor, conv, ProductInboundPayload::NoOp)
        .map_err(|e| PayloadParseError::InvalidExternalRef {
            kind: "parsed_product_inbound", reason: e.to_string(),
        })
}
```

### 1.4 Outbound rendering

**File: `crates/brassclaw_whatsapp_v2_adapter/src/render.rs`**

Reply-target format: `wa:+<e164_digits>` (e.g. `wa:+4915112345678`).
Meta `"to"` field expects digits without `+` (e.g. `4915112345678`).

```rust
//! Outbound rendering for WhatsApp Business Cloud API v2.

use brassclaw_product_adapters::{
    DeclaredEgressHost, EgressCredentialHandle, EgressHeader, EgressMethod,
    EgressPath, EgressRequest, FinalReplyView, ProgressUpdateView, ProductAdapterError,
};
use brassclaw_turns::ReplyTargetBindingRef;
use thiserror::Error;

use crate::payload::WHATSAPP_API_HOST;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WhatsAppRenderError {
    #[error("reply target `{target}` did not parse as wa:<e164>: {reason}")]
    InvalidReplyTarget { target: String, reason: String },
}

/// Parse `wa:+4915112345678` → `"4915112345678"` (Meta expects digits-only in "to")
pub fn parse_reply_target(target: &ReplyTargetBindingRef) -> Result<String, WhatsAppRenderError> {
    let raw = target.as_str();
    let stripped = raw.strip_prefix("wa:").ok_or_else(|| WhatsAppRenderError::InvalidReplyTarget {
        target: raw.to_string(),
        reason: "missing wa: prefix".into(),
    })?;
    // stripped is "+4915112345678" or "4915112345678"
    let digits = stripped.strip_prefix('+').unwrap_or(stripped);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(WhatsAppRenderError::InvalidReplyTarget {
            target: raw.to_string(),
            reason: "phone number must contain only digits after wa:[+]".into(),
        });
    }
    Ok(digits.to_string())
}

/// Build a canonical `wa:+<e164>` reply target from a sender wa_id (digits-only from Meta)
pub fn build_reply_target_binding(wa_id: &str) -> ReplyTargetBindingRef {
    // wa_id from Meta is digits-only; we store with + for canonical E.164
    let formatted = format!("wa:+{wa_id}");
    // safety: format produces digits/'+'/':' within the bounded-ref length invariant
    ReplyTargetBindingRef::new(formatted).expect("constructed reply target is well-formed")
}

/// Render a `FinalReplyView` into a sendMessage egress request.
pub fn render_final_reply(
    target: &ReplyTargetBindingRef,
    view: &FinalReplyView,
    credential_handle: EgressCredentialHandle,
    phone_number_id: &str,
) -> Result<EgressRequest, WhatsAppRenderError> {
    let to = parse_reply_target(target)?;
    let body = serde_json::json!({
        "messaging_product": "whatsapp",
        "recipient_type": "individual",
        "to": to,
        "type": "text",
        "text": {
            "body": view.text,
            "preview_url": false
        }
    });
    let body_bytes = serde_json::to_vec(&body)
        .map_err(|e| WhatsAppRenderError::InvalidReplyTarget {
            target: target.as_str().to_string(),
            reason: format!("failed to serialize reply body: {e}"),
        })?;
    let path = format!("/v19.0/{phone_number_id}/messages");
    build_egress_request(path, body_bytes, credential_handle)
}

/// Render a `ProgressUpdateView` as a "typing" reaction.
/// WhatsApp Cloud API does not have a standard typing indicator; we skip silently.
pub fn render_progress_reaction(
    _target: &ReplyTargetBindingRef,
    _view: &ProgressUpdateView,
    _credential_handle: EgressCredentialHandle,
    _phone_number_id: &str,
) -> Result<Option<EgressRequest>, WhatsAppRenderError> {
    // WhatsApp Cloud API v19 has no first-class typing indicator for business accounts.
    // Return None → adapter defers the progress push silently.
    Ok(None)
}

fn build_egress_request(
    path: String,
    body: Vec<u8>,
    credential_handle: EgressCredentialHandle,
) -> Result<EgressRequest, WhatsAppRenderError> {
    // safety: WHATSAPP_API_HOST is a compile-time const satisfying the host validator
    let host = DeclaredEgressHost::new(WHATSAPP_API_HOST).expect("static host valid");
    let method = EgressMethod::post();
    // EgressPath::new accepts any Into<String> — no static-str requirement.
    // The path "/v19.0/{phone_number_id}/messages" always starts with '/' and
    // contains no scheme, fragment, backslash, or control characters because
    // phone_number_id is digits-only (validated at store time).
    let egress_path = EgressPath::new(path).map_err(|e: ProductAdapterError| {
        WhatsAppRenderError::InvalidReplyTarget {
            target: "egress_path".into(),
            reason: e.to_string(),
        }
    })?;
    // safety: static name/value satisfies the header validator
    let content_type =
        EgressHeader::new("content-type", "application/json").expect("static header valid");
    Ok(EgressRequest::new(host, method, egress_path)
        .with_header(content_type)
        .with_body(body)
        .with_credential_handle(Some(credential_handle)))
}
```

**Note on `EgressPath::new`:** The live `EgressPath::new` signature is
`pub fn new(value: impl Into<String>) -> Result<Self, ProductAdapterError>` —
it already accepts `String` directly. No new `new_dynamic` variant is needed.

### 1.5 Tests for Phase WA-A

All tests live in the respective `src/*.rs` files under `#[cfg(test)]` or as
integration tests. Cover the same matrix as Telegram adapter tests:

**adapter.rs tests:**
- `capabilities_default_excludes_progress` — `ExternalProgressPush` absent by default
- `capabilities_with_progress_opt_in_includes_progress_push`
- `declared_egress_pairs_host_with_token_handle`
- `parse_inbound_refuses_unverified_evidence`
- `render_outbound_final_reply_uses_constrained_egress`
- `render_outbound_progress_skipped_when_capability_off`
- `render_outbound_capability_activity_deferred_without_egress`
- `render_outbound_rejects_mismatched_adapter_id`
- `render_outbound_rejects_mismatched_installation_id`
- `render_outbound_records_delivered_on_2xx`
- `render_outbound_records_retryable_on_5xx`
- `render_outbound_records_unauthorized_on_401`
- `render_outbound_records_permanent_on_400`

**payload.rs tests:**
- `parse_text_message_produces_user_message`
- `parse_status_only_produces_noop`
- `parse_non_text_type_produces_noop`
- `parse_empty_entry_produces_noop`
- `parse_missing_wamid_returns_error`
- `unverified_evidence_returns_error`
- `actor_ref_uses_e164_plus_prefix`

**render.rs tests:**
- `parse_reply_target_round_trips`
- `parse_reply_target_with_plus_prefix`
- `parse_reply_target_without_plus_prefix`
- `parse_reply_target_rejects_missing_prefix`
- `parse_reply_target_rejects_non_digits`
- `final_reply_renders_correct_json` — unwrap `Result`, check JSON body shape
- `final_reply_json_has_messaging_product_whatsapp`
- `final_reply_json_to_is_digits_only`
- `build_reply_target_binding_produces_wa_prefix`
- `progress_reaction_always_returns_none`

### 1.6 Validation for Phase WA-A

```bash
cargo test -p brassclaw_whatsapp_v2_adapter
cargo clippy -p brassclaw_whatsapp_v2_adapter --all-targets -- -D warnings
```

Both must pass with zero warnings and zero failures before marking WA-A done.

---

## 2. Phase WA-B — Database migration V062

**Goal:** Persist the WhatsApp installation configuration. One row = one Meta
WABA phone number configured by the operator. The row stores metadata only —
secrets live in the encrypted `brassclaw_secrets` store, referenced by name.

### 2.1 Migration file

**File: `crates/brassclaw_pg/migrations/V062__reborn_whatsapp_installations.sql`**

```sql
-- V062__reborn_whatsapp_installations.sql
--
-- Stores operator-configured WhatsApp Business Cloud API installations.
-- One row per WABA phone number. Secrets are NOT stored here — only the
-- secret names (whatsapp_access_token, whatsapp_app_secret, whatsapp_verify_token)
-- are referenced. The actual values live in brassclaw_secrets.
--
-- Scoped by (tenant_id, user_id, agent_id) so multi-tenant deployments can
-- have separate WhatsApp installations per agent.

CREATE TABLE IF NOT EXISTS reborn_whatsapp_installations (
    id                          UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    tenant_id                   TEXT        NOT NULL,
    user_id                     TEXT        NOT NULL,
    agent_id                    TEXT        NOT NULL DEFAULT 'default',

    -- Human-readable label shown in the WebUI ("My Business WhatsApp")
    display_name                TEXT        NOT NULL,

    -- Meta phone number ID from the Developer Console
    -- (e.g. "123456789012345"). Used as the URL path segment in egress.
    phone_number_id             TEXT        NOT NULL,

    -- E.164 display number shown to the operator, e.g. "+49 151 12345678"
    display_phone_number        TEXT        NOT NULL DEFAULT '',

    -- Secret handle names (match brassclaw_secrets key names)
    access_token_secret_name    TEXT        NOT NULL DEFAULT 'whatsapp_access_token',
    app_secret_name             TEXT        NOT NULL DEFAULT 'whatsapp_app_secret',
    verify_token_secret_name    TEXT        NOT NULL DEFAULT 'whatsapp_verify_token',

    -- Whether the installation actively receives/sends messages
    enabled                     BOOLEAN     NOT NULL DEFAULT TRUE,

    -- Feature flag: emit ExternalProgressPush
    progress_push_enabled       BOOLEAN     NOT NULL DEFAULT FALSE,

    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- One installation per (tenant, agent, phone_number_id)
    CONSTRAINT reborn_whatsapp_installations_unique
        UNIQUE (tenant_id, agent_id, phone_number_id)
);

CREATE INDEX IF NOT EXISTS reborn_whatsapp_installations_tenant_agent_idx
    ON reborn_whatsapp_installations (tenant_id, agent_id);

CREATE INDEX IF NOT EXISTS reborn_whatsapp_installations_enabled_idx
    ON reborn_whatsapp_installations (tenant_id, agent_id, enabled)
    WHERE enabled = TRUE;

CREATE TRIGGER reborn_whatsapp_installations_updated_at
    BEFORE UPDATE ON reborn_whatsapp_installations
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

### 2.2 Rust store type

**File: `crates/brassclaw_reborn_composition/src/whatsapp_installation_store.rs`**

A simple Postgres-backed store, scoped to `(tenant_id, user_id, agent_id)`.

```rust
//! Postgres-backed store for reborn_whatsapp_installations.

use std::sync::Arc;

use brassclaw_pg::PgPool;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppInstallation {
    pub id: Uuid,
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub display_name: String,
    pub phone_number_id: String,
    pub display_phone_number: String,
    pub access_token_secret_name: String,
    pub app_secret_name: String,
    pub verify_token_secret_name: String,
    pub enabled: bool,
    pub progress_push_enabled: bool,
}

#[derive(Debug, Error)]
pub enum WhatsAppInstallationStoreError {
    #[error("database error: {0}")]
    Db(#[from] tokio_postgres::Error),
    #[error("pool error: {0}")]
    Pool(String),
    #[error("installation not found")]
    NotFound,
}

pub struct WhatsAppInstallationStore {
    pool: Arc<PgPool>,
}

impl WhatsAppInstallationStore {
    pub fn new(pool: Arc<PgPool>) -> Self { Self { pool } }

    async fn connect(&self) -> Result<deadpool_postgres::Object, WhatsAppInstallationStoreError> {
        self.pool.get().await.map_err(|e| WhatsAppInstallationStoreError::Pool(e.to_string()))
    }

    /// Load all enabled installations for a (tenant, agent) pair.
    /// Called at startup to build the live adapter registry.
    pub async fn list_enabled(
        &self,
        tenant_id: &str,
        agent_id: &str,
    ) -> Result<Vec<WhatsAppInstallation>, WhatsAppInstallationStoreError> {
        let client = self.connect().await?;
        let rows = client.query(
            "SELECT id, tenant_id, user_id, agent_id, display_name, phone_number_id,
                    display_phone_number, access_token_secret_name, app_secret_name,
                    verify_token_secret_name, enabled, progress_push_enabled
             FROM reborn_whatsapp_installations
             WHERE tenant_id = $1 AND agent_id = $2 AND enabled = TRUE
             ORDER BY created_at ASC",
            &[&tenant_id, &agent_id],
        ).await?;
        rows.iter().map(row_to_installation).collect()
    }

    /// Load a single installation by id (for the settings API).
    pub async fn get(
        &self,
        tenant_id: &str,
        id: &Uuid,
    ) -> Result<WhatsAppInstallation, WhatsAppInstallationStoreError> {
        let client = self.connect().await?;
        let row = client.query_opt(
            "SELECT id, tenant_id, user_id, agent_id, display_name, phone_number_id,
                    display_phone_number, access_token_secret_name, app_secret_name,
                    verify_token_secret_name, enabled, progress_push_enabled
             FROM reborn_whatsapp_installations
             WHERE tenant_id = $1 AND id = $2",
            &[&tenant_id, id],
        ).await?.ok_or(WhatsAppInstallationStoreError::NotFound)?;
        row_to_installation(&row)
    }

    /// Upsert an installation (used by the settings endpoint).
    pub async fn upsert(
        &self,
        inst: &WhatsAppInstallation,
    ) -> Result<(), WhatsAppInstallationStoreError> {
        let client = self.connect().await?;
        client.execute(
            "INSERT INTO reborn_whatsapp_installations
                (id, tenant_id, user_id, agent_id, display_name, phone_number_id,
                 display_phone_number, access_token_secret_name, app_secret_name,
                 verify_token_secret_name, enabled, progress_push_enabled)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
             ON CONFLICT (tenant_id, agent_id, phone_number_id) DO UPDATE SET
                display_name             = EXCLUDED.display_name,
                display_phone_number     = EXCLUDED.display_phone_number,
                access_token_secret_name = EXCLUDED.access_token_secret_name,
                app_secret_name          = EXCLUDED.app_secret_name,
                verify_token_secret_name = EXCLUDED.verify_token_secret_name,
                enabled                  = EXCLUDED.enabled,
                progress_push_enabled    = EXCLUDED.progress_push_enabled,
                updated_at               = now()",
            &[
                &inst.id, &inst.tenant_id, &inst.user_id, &inst.agent_id,
                &inst.display_name, &inst.phone_number_id, &inst.display_phone_number,
                &inst.access_token_secret_name, &inst.app_secret_name,
                &inst.verify_token_secret_name, &inst.enabled, &inst.progress_push_enabled,
            ],
        ).await?;
        Ok(())
    }

    /// Delete an installation (disable + remove row).
    pub async fn delete(
        &self,
        tenant_id: &str,
        id: &Uuid,
    ) -> Result<(), WhatsAppInstallationStoreError> {
        let client = self.connect().await?;
        client.execute(
            "DELETE FROM reborn_whatsapp_installations WHERE tenant_id = $1 AND id = $2",
            &[&tenant_id, id],
        ).await?;
        Ok(())
    }
}

fn row_to_installation(
    row: &tokio_postgres::Row,
) -> Result<WhatsAppInstallation, WhatsAppInstallationStoreError> {
    Ok(WhatsAppInstallation {
        id:                        row.get(0),
        tenant_id:                 row.get(1),
        user_id:                   row.get(2),
        agent_id:                  row.get(3),
        display_name:              row.get(4),
        phone_number_id:           row.get(5),
        display_phone_number:      row.get(6),
        access_token_secret_name:  row.get(7),
        app_secret_name:           row.get(8),
        verify_token_secret_name:  row.get(9),
        enabled:                   row.get(10),
        progress_push_enabled:     row.get(11),
    })
}
```

Add to `crates/brassclaw_reborn_composition/src/lib.rs`:

```rust
#[cfg(feature = "postgres")]
pub(crate) mod whatsapp_installation_store;
#[cfg(feature = "postgres")]
pub(crate) use whatsapp_installation_store::{
    WhatsAppInstallation, WhatsAppInstallationStore, WhatsAppInstallationStoreError,
};
```

Place this alongside the other `#[cfg(feature = "postgres")]` DB store modules
(e.g. near `pg_monty_vm_settings`, `pg_user_preference_store`).

### 2.3 Validation for Phase WA-B

Apply the migration and confirm:

```bash
# Apply migration
cargo run --bin brassclaw -- db migrate

# Smoke-test the table exists
psql $BRASSCLAW_PG_URL -c "\d reborn_whatsapp_installations"

# Compile and lint the store
cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings
cargo test -p brassclaw_reborn_composition
```

---

## 3. Phase WA-C — Ingress routes

**Goal:** Register three new HTTP routes that Meta needs:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/webhooks/whatsapp` | Meta hub challenge verification (one-time at setup) |
| `POST` | `/webhooks/whatsapp` | Live inbound webhook (every message) |
| `GET` | `/api/webchat/v2/channels/whatsapp/status` | WebUI health/status (new) |

The GET challenge handler must **not** go through `ProductAdapter::parse_inbound`.
It is a pure host concern — verify `hub.verify_token`, echo `hub.challenge`.

### 3.1 Auth requirement for POST webhook

WhatsApp uses HMAC-SHA256:

```
X-Hub-Signature-256: sha256=<hex_digest>
```

The digest is `HMAC-SHA256(app_secret, raw_request_body)`.
The existing `HmacWebhookAuth` verifier in `brassclaw_product_adapters::auth_verifier`
handles HMAC-SHA256 already (it is used by Slack). The `AuthRequirement` variant for
this is `RequestSignature { header_name: "X-Hub-Signature-256", timestamp_header_name: None }`.

The host verifier:
1. Reads raw body bytes.
2. Reads header `X-Hub-Signature-256`.
3. Strips `sha256=` prefix from header value.
4. Computes `HMAC-SHA256(whatsapp_app_secret, body)`.
5. Compares with constant-time equality (`subtle::ConstantTimeEq`).
6. On match → mints `ProtocolAuthEvidence::Verified`.
7. Calls `WhatsAppV2Adapter::parse_inbound(body, verified_evidence)`.

### 3.2 GET challenge handler

The handler lives in `brassclaw_reborn_composition/src/whatsapp.rs`.

```rust
/// Meta webhook verification handshake.
///
/// Meta hits this endpoint with:
///   ?hub.mode=subscribe&hub.verify_token=<token>&hub.challenge=<challenge>
///
/// We verify hub.verify_token matches the configured whatsapp_verify_token
/// secret (constant-time), then echo hub.challenge as plain text 200.
/// On mismatch: 403.
pub async fn handle_whatsapp_challenge(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<WhatsAppWebhookState>,
) -> impl IntoResponse {
    let mode     = params.get("hub.mode").map(String::as_str).unwrap_or("");
    let token    = params.get("hub.verify_token").map(String::as_str).unwrap_or("");
    let challenge = params.get("hub.challenge").map(String::as_str).unwrap_or("");

    if mode != "subscribe" {
        return (StatusCode::FORBIDDEN, "mode must be subscribe").into_response();
    }

    // Resolve the configured verify_token from the secret store.
    // Use constant-time comparison.
    let expected = match state.resolve_verify_token().await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "").into_response(),
    };

    let token_bytes = token.as_bytes();
    let expected_bytes = expected.as_bytes();
    let len_ok = token_bytes.len() == expected_bytes.len();
    // Pad to same length before constant-time compare to avoid length-leak
    let cmp_len = token_bytes.len().max(expected_bytes.len());
    let mut t = vec![0u8; cmp_len];
    let mut e = vec![0u8; cmp_len];
    t[..token_bytes.len()].copy_from_slice(token_bytes);
    e[..expected_bytes.len()].copy_from_slice(expected_bytes);
    let ct_match = bool::from(t.ct_eq(&e));

    if len_ok && ct_match {
        (StatusCode::OK, challenge.to_string()).into_response()
    } else {
        (StatusCode::FORBIDDEN, "verify_token mismatch").into_response()
    }
}
```

`WhatsAppWebhookState` wraps a reference to the secret store so it can resolve
`whatsapp_verify_token` at request time without caching it in memory.

### 3.3 POST webhook handler

```rust
/// Inbound WhatsApp message webhook.
///
/// 1. The HMAC middleware (see §3.4) verifies X-Hub-Signature-256, then injects
///    a `ProtocolAuthEvidence::Verified` value via `axum::Extension` before
///    calling this handler.
/// 2. Parse the verified body via `WhatsAppV2Adapter::parse_inbound`.
/// 3. Wrap into a `ProductInboundEnvelope` via `TrustedInboundContext`.
/// 4. Submit to `ProductWorkflow`. Always return 200 — Meta retries on non-200.
pub async fn handle_whatsapp_inbound(
    State(state): State<WhatsAppWebhookState>,
    Extension(evidence): Extension<ProtocolAuthEvidence>,
    body: Bytes,
) -> impl IntoResponse {
    use brassclaw_product_adapters::{ProductInboundEnvelope, TrustedInboundContext};
    use chrono::Utc;

    let result = state.adapter.parse_inbound(&body, &evidence);
    match result {
        Ok(parsed) => {
            // Stamp the trusted context (adapter id, installation id, auth claim)
            // before handing the envelope to the workflow.
            let context = match TrustedInboundContext::from_verified_evidence(
                state.adapter.adapter_id().clone(),
                state.adapter.installation_id().clone(),
                Utc::now(),
                &evidence,
            ) {
                Ok(ctx) => ctx,
                Err(e) => {
                    tracing::debug!(error = %e, "whatsapp inbound: failed to stamp trusted context");
                    return StatusCode::OK;
                }
            };
            let envelope = match ProductInboundEnvelope::from_trusted_parse(context, parsed) {
                Ok(env) => env,
                Err(e) => {
                    tracing::debug!(error = %e, "whatsapp inbound: envelope construction failed");
                    return StatusCode::OK;
                }
            };
            if let Err(e) = state.workflow.submit_inbound(envelope).await {
                // Debug only — info/warn corrupts the TUI.
                tracing::debug!(error = %e, "whatsapp inbound: workflow submission failed");
            }
        }
        Err(ProductAdapterError::Authentication(_)) => {
            // Should not reach here — HMAC middleware already verified.
            // Log and fall through to 200 so Meta does not retry.
            tracing::debug!("whatsapp inbound: authentication error after middleware — dropped");
        }
        Err(e) => {
            // Malformed payload. Return 200 so Meta does not retry.
            tracing::debug!(error = %e, "whatsapp inbound: malformed payload — dropped");
        }
    }

    StatusCode::OK
}
```

### 3.4 Route wiring in `webui_serve.rs`

**File to modify:** `crates/brassclaw_reborn_composition/src/webui_serve.rs`

Add a sub-router for the WhatsApp webhook surface, feature-gated on
`REBORN_WHATSAPP_V2_ENABLED=true` (env var checked at startup):

```rust
// In webui_v2_app() or the equivalent router-building function:
if std::env::var("REBORN_WHATSAPP_V2_ENABLED").as_deref() == Ok("true") {
    router = router
        .route("/webhooks/whatsapp",
            get(whatsapp::handle_whatsapp_challenge)
            .post(whatsapp::handle_whatsapp_inbound_with_hmac_middleware)
        );
}
```

The POST route is wrapped with an axum middleware layer that:
1. Buffers the raw body (required for HMAC — once read, the body stream is consumed).
2. Resolves `whatsapp_app_secret` from the secret store.
3. Computes `HMAC-SHA256(secret, body)`.
4. Compares to `X-Hub-Signature-256` header with constant-time equality.
5. On match: injects `ProtocolAuthEvidence::Verified` as an `axum::Extension` and calls next.
6. On mismatch: returns `403 Forbidden` immediately.

**Security invariant:** The HMAC key (`whatsapp_app_secret`) is resolved from the
secret store at request time and never stored in process memory across requests.

### 3.5 Route descriptors

**File to modify:** `crates/brassclaw_webui_v2/src/descriptors.rs`

Add:

```rust
// WhatsApp channel routes
pub const WEBUI_V2_ROUTE_WHATSAPP_WEBHOOK_CHALLENGE: &str = "webui.v2.whatsapp_webhook_challenge";
pub const WEBUI_V2_ROUTE_WHATSAPP_WEBHOOK_INBOUND: &str = "webui.v2.whatsapp_webhook_inbound";
pub const WEBUI_V2_ROUTE_WHATSAPP_SETTINGS_GET: &str = "webui.v2.whatsapp_settings_get";
pub const WEBUI_V2_ROUTE_WHATSAPP_SETTINGS_UPSERT: &str = "webui.v2.whatsapp_settings_upsert";
pub const WEBUI_V2_ROUTE_WHATSAPP_SETTINGS_DELETE: &str = "webui.v2.whatsapp_settings_delete";

pub const WEBUI_V2_PATTERN_WHATSAPP_WEBHOOK: &str = "/webhooks/whatsapp";
pub const WEBUI_V2_PATTERN_WHATSAPP_SETTINGS: &str = "/api/webchat/v2/channels/whatsapp";
pub const WEBUI_V2_PATTERN_WHATSAPP_SETTINGS_ID: &str = "/api/webchat/v2/channels/whatsapp/{installation_id}";
```

### 3.6 Validation for Phase WA-C

```bash
cargo build -p brassclaw_reborn_composition
cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings
cargo test -p brassclaw_reborn_composition
```

---

## 4. Phase WA-D — Composition wiring + secrets

**Goal:** Wire the WhatsApp adapter into the live composition path so that when
`REBORN_WHATSAPP_V2_ENABLED=true` the adapter is active and inbound messages
flow to the agent loop.

### 4.1 New module `whatsapp.rs` in composition

**File: `crates/brassclaw_reborn_composition/src/whatsapp.rs`**

This module owns:
- Loading `WhatsAppInstallation` rows from DB at startup.
- Constructing `WhatsAppV2Adapter` instances.
- Registering them in the `ProductAdapterRegistry`.
- Providing the `WhatsAppWebhookState` for the HTTP handlers.
- The `validate_whatsapp_v1_v2_exclusivity()` guard.

```rust
use std::sync::Arc;

use brassclaw_product_adapters::{
    AdapterInstallationId, AuthRequirement, EgressCredentialHandle, ProductAdapterId,
};
use brassclaw_whatsapp_v2_adapter::{WhatsAppV2Adapter, WhatsAppV2AdapterConfig};

use crate::whatsapp_installation_store::{WhatsAppInstallation, WhatsAppInstallationStore};

// ── WhatsAppWebhookState ─────────────────────────────────────────────────────
//
// Shared state injected into the axum webhook handlers (§3.2, §3.3).
// Cheap to clone (all fields are Arc-wrapped).

/// State passed to the GET challenge and POST inbound handlers.
#[derive(Clone)]
pub struct WhatsAppWebhookState {
    /// Live adapter for the active installation. The host resolves secrets
    /// at request time via the adapter's credential handles — no raw secrets here.
    pub adapter: Arc<WhatsAppV2Adapter>,
    /// Product workflow: accepts parsed inbound envelopes.
    pub workflow: Arc<dyn brassclaw_product_adapters::ProductWorkflow>,
    /// Secret store: used only to resolve `whatsapp_verify_token` in the GET handler.
    pub secret_store: Arc<dyn brassclaw_secrets::SecretStore>,
}

impl WhatsAppWebhookState {
    /// Resolve the configured `whatsapp_verify_token` from the secret store
    /// at request time. Returns an error if the secret is absent.
    pub async fn resolve_verify_token(&self) -> Result<String, String> {
        let secret_name = self.adapter.config().verify_token_secret_name();
        self.secret_store
            .get(secret_name)
            .await
            .map(|s| s.expose_secret().to_string())
            .map_err(|e| e.to_string())
    }
}

/// Build a `WhatsAppV2Adapter` from a stored installation record.
///
/// The adapter does not hold the secret values — only the handles (names).
/// The host resolves names → values at egress/verification time.
pub fn build_adapter_from_installation(
    inst: &WhatsAppInstallation,
) -> Result<WhatsAppV2Adapter, String> {
    let adapter_id = ProductAdapterId::new("whatsapp_v2")
        .map_err(|e| e.to_string())?;
    let installation_id = AdapterInstallationId::new(&inst.id.to_string())
        .map_err(|e| e.to_string())?;
    let egress_credential_handle =
        EgressCredentialHandle::new(&inst.access_token_secret_name)
            .map_err(|e| e.to_string())?;

    // WhatsApp uses HMAC-SHA256 over the raw body.
    // The header is X-Hub-Signature-256.
    let auth_requirement = AuthRequirement::RequestSignature {
        header_name: "X-Hub-Signature-256".into(),
        timestamp_header_name: None,
    };

    Ok(WhatsAppV2Adapter::new(WhatsAppV2AdapterConfig {
        adapter_id,
        installation_id,
        phone_number_id: inst.phone_number_id.clone(),
        egress_credential_handle,
        auth_requirement,
        progress_push_enabled: inst.progress_push_enabled,
        verify_token_secret_name: inst.verify_token_secret_name.clone(),
    }))
}

/// At startup, if REBORN_WHATSAPP_V2_ENABLED is set and v1 channel artifacts
/// are also present, abort with a clear message.
///
/// Mirrors `validate_telegram_v1_v2_exclusivity`. This is a startup-safety
/// guard — intentional abort is acceptable here (same class as other
/// "invalid operator configuration" guards in the composition crate).
pub fn validate_whatsapp_v1_v2_exclusivity(v1_channel_artifacts_present: bool) {
    let v2_enabled = std::env::var("REBORN_WHATSAPP_V2_ENABLED").as_deref() == Ok("true");
    if v2_enabled && v1_channel_artifacts_present {
        // safety: startup configuration invariant — v1 and v2 cannot coexist.
        // This matches the abort-on-invalid-config pattern used by other
        // composition-crate startup guards.
        panic!(
            "REBORN_WHATSAPP_V2_ENABLED=true is set but v1 WhatsApp channel artifacts \
             are also present. Remove the v1 channel configuration before enabling v2."
        );
    }
}
```

### 4.2 Wiring into `factory.rs` / startup

**File to modify:** `crates/brassclaw_reborn_composition/src/factory.rs`

In the Reborn service builder, after DB is up:

```rust
#[cfg(feature = "postgres")]
if std::env::var("REBORN_WHATSAPP_V2_ENABLED").as_deref() == Ok("true") {
    validate_whatsapp_v1_v2_exclusivity(false); // pass v1 detection result
    let store = WhatsAppInstallationStore::new(Arc::clone(&pg_pool));
    let installations = store.list_enabled(&tenant_id, &agent_id).await
        .map_err(|e| RebornBuildError::Config(e.to_string()))?;
    for inst in &installations {
        let adapter = build_adapter_from_installation(inst)
            .map_err(|e| RebornBuildError::Config(e))?;
        adapter_registry.register(Arc::new(adapter));
    }
}
```

### 4.3 `StaticConnectableChannelsProductFacade` registration

**File to modify:** `crates/brassclaw_product_workflow/src/reborn_services.rs`

Add WhatsApp to the static list alongside Telegram/Slack. The connect strategy
is `AdminManagedChannels` — setup requires operator action in the Settings page,
not a user-side pairing flow:

```rust
RebornConnectableChannelInfo {
    channel: "whatsapp".into(),
    display_name: "WhatsApp".into(),
    strategy: RebornChannelConnectStrategy::AdminManagedChannels,
    action: RebornChannelConnectAction {
        title: "Connect WhatsApp Business Account".into(),
        instructions: "Save your Meta system user access token, app secret, \
                       and webhook verify token in Settings → Channels → WhatsApp.".into(),
        input_placeholder: "System user access token".into(),
        submit_label: "Save".into(),
        success_message: "WhatsApp Business connected".into(),
        error_message: "Check your credentials and try again".into(),
    },
    command_aliases: vec![],
}
```

### 4.4 Secrets environment variables

The three secrets must be resolvable by name from the `brassclaw_secrets` store.
Add them to the operator-tier documentation (not config files):

| Secret name | Env var (bootstrap fallback) | Notes |
|---|---|---|
| `whatsapp_access_token` | `WHATSAPP_ACCESS_TOKEN` | Meta system user token (Bearer) |
| `whatsapp_app_secret` | `WHATSAPP_APP_SECRET` | HMAC key for X-Hub-Signature-256 |
| `whatsapp_verify_token` | `WHATSAPP_VERIFY_TOKEN` | GET challenge verification secret |

These are never persisted in `config.toml`. The secrets store resolves them from
the encrypted store at request time.

### 4.5 Feature flag: `REBORN_WHATSAPP_V2_ENABLED`

Default: **off**. The Telegram pattern (`REBORN_TELEGRAM_V2_ENABLED`) is the model.

When `false` (default):
- No webhook routes are registered.
- No adapter is built.
- The `brassclaw_whatsapp_v2_adapter` crate compiles but is idle.
- The `reborn_whatsapp_installations` table exists but is unused.

When `true`:
- All routes registered.
- All enabled DB installations are loaded into the adapter registry.
- Inbound messages flow normally.

### 4.6 Validation for Phase WA-D

```bash
cargo build --release --bin brassclaw
cargo clippy --all --benches --tests --examples --all-features -- -D warnings
cargo test -p brassclaw_reborn_composition
```

---

## 5. Phase WA-E — TraceChannel + outbound delivery

**Goal:** Add `TraceChannel::WhatsApp` so traces and logs correctly attribute
WhatsApp-sourced turns. Wire the outbound delivery preference so users can select
WhatsApp as a reply target.

### 5.1 `TraceChannel::WhatsApp`

**File to modify:** `crates/brassclaw_reborn_traces/src/contribution.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TraceChannel {
    Web,
    Cli,
    Telegram,
    Slack,
    WhatsApp,   // ← ADD
    Routine,
    Other,
}
```

Also update the exhaustive `channel_label()` match in the same file:

```rust
fn channel_label(channel: TraceChannel) -> &'static str {
    match channel {
        TraceChannel::Web => "web",
        TraceChannel::Cli => "cli",
        TraceChannel::Telegram => "telegram",
        TraceChannel::Slack => "slack",
        TraceChannel::WhatsApp => "whatsapp",  // ← ADD
        TraceChannel::Routine => "routine",
        TraceChannel::Other => "other",
    }
}
```

Also update the `TraceChannelArg` enum and its `From<TraceChannelArg>` impl in
`crates/brassclaw_reborn_cli/src/commands/traces/mod.rs`:

```rust
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceChannelArg {
    Web,
    Cli,
    Telegram,
    Slack,
    WhatsApp,   // ← ADD
    Routine,
    Other,
}

impl std::fmt::Display for TraceChannelArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Web => "web",
            Self::Cli => "cli",
            Self::Telegram => "telegram",
            Self::Slack => "slack",
            Self::WhatsApp => "whatsapp",  // ← ADD
            Self::Routine => "routine",
            Self::Other => "other",
        };
        write!(f, "{value}")
    }
}

impl From<TraceChannelArg> for TraceChannel {
    fn from(value: TraceChannelArg) -> Self {
        match value {
            TraceChannelArg::Web => TraceChannel::Web,
            TraceChannelArg::Cli => TraceChannel::Cli,
            TraceChannelArg::Telegram => TraceChannel::Telegram,
            TraceChannelArg::Slack => TraceChannel::Slack,
            TraceChannelArg::WhatsApp => TraceChannel::WhatsApp,  // ← ADD
            TraceChannelArg::Routine => TraceChannel::Routine,
            TraceChannelArg::Other => TraceChannel::Other,
        }
    }
}
```

**File to modify:** `crates/brassclaw_reborn_traces/src/client.rs`

In `trace_channel_from_host_channel(channel: &str) -> TraceChannel`:

```rust
"whatsapp" | "whatsapp_v2" => TraceChannel::WhatsApp,
```

### 5.2 Outbound delivery target for WhatsApp

When the agent sends a reply to a WhatsApp message, the
`OutboundResolutionEngine` (from `docs/superpowers/specs/2026-05-29-channel-communication-delivery-resolution.md`)
resolves the candidate delivery target. For WhatsApp this is:

- Target type: `RebornOutboundDeliveryTargetChannel` with value `"whatsapp"`
- Capabilities: `final_replies: true, gate_prompts: false, auth_prompts: false`

In `brassclaw_outbound` (the `OutboundPolicyService`), ensure that the
`"whatsapp"` channel string is accepted by the `RebornOutboundDeliveryTargetChannel`
validator. It already accepts any non-empty string ≤128 bytes free of control chars —
no code change needed there.

The `outbound_preferences` WebUI endpoint that lets a user set their preferred
reply channel already exists (`/api/webchat/v2/outbound-preferences`). No change
needed to make WhatsApp a valid choice there — the channel value `"whatsapp"` flows
through as an opaque string.

### 5.3 Validation for Phase WA-E

```bash
cargo test -p brassclaw_reborn_traces
cargo clippy -p brassclaw_reborn_traces --all-targets -- -D warnings
cargo test -p brassclaw_outbound
```

---

## 6. Phase WA-F — WebUI v2 Settings page

**Goal:** The WebUI Settings panel gets a new "Channels" section with a
WhatsApp card. The operator can:

1. View the current WhatsApp installation status (connected / not connected).
2. Enter/update the three secrets (phone number ID, access token, app secret, verify token).
3. Enable/disable the installation.
4. See the webhook URL to paste into the Meta Developer Console.

This is a pure settings surface — no changes to the agent loop or thread model.

### 6.1 Backend API routes

**File to modify:** `crates/brassclaw_webui_v2/src/descriptors.rs`

The patterns were added in Phase WA-C (§3.5). Now add the full descriptors:

```rust
fn whatsapp_settings_get_descriptor() -> IngressRouteDescriptor {
    IngressRouteDescriptor::builder()
        .route_id(WEBUI_V2_ROUTE_WHATSAPP_SETTINGS_GET)
        .method(NetworkMethod::Get)
        .pattern(WEBUI_V2_PATTERN_WHATSAPP_SETTINGS)
        .auth(IngressAuthPolicy::Required(IngressAuthScheme::Bearer))
        .body_limit(BodyLimitPolicy::NoBody)
        .rate_limit(RateLimitPolicy::PerCaller {
            requests_per_minute: NonZeroU32::new(60).expect("nonzero"), // safety: crate-local positive constant
            scope: RateLimitScope::User,
        })
        .cors(CorsPolicy::SameOriginOnly)
        .audit(AuditTraceClass::ReadOnly)
        .build()
}

fn whatsapp_settings_upsert_descriptor() -> IngressRouteDescriptor {
    IngressRouteDescriptor::builder()
        .route_id(WEBUI_V2_ROUTE_WHATSAPP_SETTINGS_UPSERT)
        .method(NetworkMethod::Post)
        .pattern(WEBUI_V2_PATTERN_WHATSAPP_SETTINGS)
        .auth(IngressAuthPolicy::Required(IngressAuthScheme::Bearer))
        .body_limit(BodyLimitPolicy::JsonBytes(4096))
        .rate_limit(RateLimitPolicy::PerCaller {
            requests_per_minute: NonZeroU32::new(10).expect("nonzero"), // safety: crate-local positive constant
            scope: RateLimitScope::User,
        })
        .cors(CorsPolicy::SameOriginOnly)
        .audit(AuditTraceClass::Mutating)
        .build()
}

fn whatsapp_settings_delete_descriptor() -> IngressRouteDescriptor {
    IngressRouteDescriptor::builder()
        .route_id(WEBUI_V2_ROUTE_WHATSAPP_SETTINGS_DELETE)
        .method(NetworkMethod::Delete)
        .pattern(WEBUI_V2_PATTERN_WHATSAPP_SETTINGS_ID)
        .auth(IngressAuthPolicy::Required(IngressAuthScheme::Bearer))
        .body_limit(BodyLimitPolicy::NoBody)
        .rate_limit(RateLimitPolicy::PerCaller {
            requests_per_minute: NonZeroU32::new(10).expect("nonzero"), // safety: crate-local positive constant
            scope: RateLimitScope::User,
        })
        .cors(CorsPolicy::SameOriginOnly)
        .audit(AuditTraceClass::Mutating)
        .build()
}
```

Add all three to `webui_v2_routes()` return vector.

### 6.2 Handler implementations

**File: `crates/brassclaw_webui_v2/src/handlers/whatsapp_settings.rs`** (new file)

```rust
use axum::Json;
use axum::extract::{Extension, Path, State};
use brassclaw_product_workflow::{RebornServicesApi, WebUiAuthenticatedCaller};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// GET /api/webchat/v2/channels/whatsapp
/// Returns all WhatsApp installations for the caller's tenant.
pub async fn get_whatsapp_settings(
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    State(state): State<crate::router::WebUiV2State>,
) -> impl axum::response::IntoResponse {
    let response = state.services()
        .get_whatsapp_installations(caller)
        .await;
    crate::handlers::map_result_to_json(response)
}

/// POST /api/webchat/v2/channels/whatsapp
/// Create or update a WhatsApp installation.
pub async fn upsert_whatsapp_settings(
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    State(state): State<crate::router::WebUiV2State>,
    Json(request): Json<UpsertWhatsAppInstallationRequest>,
) -> impl axum::response::IntoResponse {
    let response = state.services()
        .upsert_whatsapp_installation(caller, request)
        .await;
    crate::handlers::map_result_to_json(response)
}

/// DELETE /api/webchat/v2/channels/whatsapp/{installation_id}
pub async fn delete_whatsapp_settings(
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    State(state): State<crate::router::WebUiV2State>,
    Path(installation_id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let response = state.services()
        .delete_whatsapp_installation(caller, installation_id)
        .await;
    crate::handlers::map_result_to_json(response)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertWhatsAppInstallationRequest {
    pub display_name: String,
    pub phone_number_id: String,
    pub display_phone_number: String,
    /// If provided, updates the access token secret in the secret store.
    /// Never returned in GET responses (secrets are write-only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// If provided, updates the app secret in the secret store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_secret: Option<String>,
    /// If provided, updates the verify token in the secret store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_token: Option<String>,
    pub enabled: bool,
    pub progress_push_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppInstallationView {
    pub id: Uuid,
    pub display_name: String,
    pub phone_number_id: String,
    pub display_phone_number: String,
    /// Whether the access token secret is configured (not the value itself).
    pub access_token_configured: bool,
    pub app_secret_configured: bool,
    pub verify_token_configured: bool,
    pub enabled: bool,
    pub progress_push_enabled: bool,
    /// The webhook URL this installation expects Meta to POST to.
    /// e.g. "https://your-domain.com/webhooks/whatsapp"
    pub webhook_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppInstallationsResponse {
    pub installations: Vec<WhatsAppInstallationView>,
}
```

**Security invariant on secrets in settings API:**
- `GET` response NEVER returns secret values. It returns only `*_configured: bool`.
- `POST` body MAY include new secret values (to update them). The handler writes
  them to the encrypted secret store and sets `*_configured: true`.
- The `access_token`, `app_secret`, `verify_token` fields in the response are
  always absent (`skip_serializing_if = "Option::is_none"` + never set on GET).

### 6.3 Facade method on `RebornServicesApi`

**File to modify:** `crates/brassclaw_product_workflow/src/reborn_services.rs`

Add three methods to the `RebornServicesApi` trait:

```rust
async fn get_whatsapp_installations(
    &self,
    caller: WebUiAuthenticatedCaller,
) -> Result<WhatsAppInstallationsResponse, RebornServicesError>;

async fn upsert_whatsapp_installation(
    &self,
    caller: WebUiAuthenticatedCaller,
    request: UpsertWhatsAppInstallationRequest,
) -> Result<WhatsAppInstallationView, RebornServicesError>;

async fn delete_whatsapp_installation(
    &self,
    caller: WebUiAuthenticatedCaller,
    installation_id: Uuid,
) -> Result<(), RebornServicesError>;
```

The concrete implementation in `RebornServices` calls through to
`WhatsAppInstallationStore` (gated on `#[cfg(feature = "postgres")]`).
Secret values in the `UpsertWhatsAppInstallationRequest` are written to the
secret store via the existing secrets API. The implementation checks
`caller.tenant_id()` scoping on every DB call.

### 6.4 Router wiring

**File to modify:** `crates/brassclaw_webui_v2/src/router.rs`

```rust
use crate::handlers::whatsapp_settings;

// Inside webui_v2_router_with_options():
.route(
    WEBUI_V2_PATTERN_WHATSAPP_SETTINGS,
    get(whatsapp_settings::get_whatsapp_settings)
        .post(whatsapp_settings::upsert_whatsapp_settings),
)
.route(
    WEBUI_V2_PATTERN_WHATSAPP_SETTINGS_ID,
    delete(whatsapp_settings::delete_whatsapp_settings),
)
```

Add `mod whatsapp_settings;` to `crates/brassclaw_webui_v2/src/handlers.rs`.

### 6.5 Validation for Phase WA-F

```bash
cargo test -p brassclaw_webui_v2
cargo test -p brassclaw_product_workflow
cargo clippy -p brassclaw_webui_v2 --all-targets -- -D warnings
cargo clippy -p brassclaw_product_workflow --all-targets -- -D warnings
```

---

## 7. Phase WA-G — Integration tests + full validation

**Goal:** Prove the end-to-end path works: a synthetic webhook POST flows through
the full ingress → adapter → workflow stack, creates a thread, and a rendered
outbound reply is produced.

### 7.1 Integration test: inbound webhook → thread creation

**File: `tests/e2e/whatsapp_inbound_integration.rs`** (or
`crates/brassclaw_reborn_composition/tests/whatsapp_webhook_contract.rs`)

```rust
/// Sends a synthetic, HMAC-signed WhatsApp webhook POST to a test Reborn server
/// and verifies:
/// 1. Response is 200 OK.
/// 2. A SessionThread is created with the correct external actor ref.
/// 3. The thread is visible in the WebUI /api/webchat/v2/threads list.
#[tokio::test]
#[cfg(feature = "integration")]
async fn whatsapp_inbound_creates_thread() {
    // 1. Start a test Reborn server with REBORN_WHATSAPP_V2_ENABLED=true
    // 2. Insert a test WhatsAppInstallation row in the DB
    // 3. Construct a synthetic Meta webhook payload
    // 4. Compute HMAC-SHA256(test_app_secret, body)
    // 5. POST /webhooks/whatsapp with X-Hub-Signature-256: sha256=<digest>
    // 6. Assert 200 OK
    // 7. Assert SessionThread exists with external_actor_ref.kind = "whatsapp_user"
    // 8. Assert the thread appears in GET /api/webchat/v2/threads
}
```

### 7.2 Integration test: GET challenge verification

```rust
#[tokio::test]
#[cfg(feature = "integration")]
async fn whatsapp_challenge_verification_succeeds() {
    // GET /webhooks/whatsapp?hub.mode=subscribe&hub.verify_token=<correct>&hub.challenge=abc123
    // Assert: 200 OK, body = "abc123"
}

#[tokio::test]
#[cfg(feature = "integration")]
async fn whatsapp_challenge_verification_rejects_wrong_token() {
    // GET /webhooks/whatsapp?hub.mode=subscribe&hub.verify_token=wrong&hub.challenge=abc123
    // Assert: 403 Forbidden
}
```

### 7.3 Integration test: outbound render

```rust
#[tokio::test]
async fn whatsapp_render_outbound_round_trip() {
    // Uses FakeProtocolHttpEgress + FakeOutboundDeliverySink
    // Verifies:
    // - POST to graph.facebook.com
    // - Path is /v19.0/{phone_number_id}/messages
    // - Body has messaging_product=whatsapp, to=<digits>, type=text
    // - DeliveryStatus::Delivered recorded
}
```

### 7.4 Architecture boundary test

**File to modify:** `crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs`

Add `brassclaw_whatsapp_v2_adapter` to the same boundary-checked list as
`brassclaw_telegram_v2_adapter`. Verify it does NOT depend on:
`brassclaw_dispatcher`, `brassclaw_capabilities`, `brassclaw_host_runtime`,
`brassclaw_network`, `brassclaw_secrets`, `brassclaw_filesystem`.

### 7.5 Final validation checklist

Run in order. All must pass before WhatsApp integration is considered done.

```bash
# 1. Unit tests for the new adapter crate
cargo test -p brassclaw_whatsapp_v2_adapter

# 2. Composition tests
cargo test -p brassclaw_reborn_composition

# 3. WebUI v2 handler tests
cargo test -p brassclaw_webui_v2

# 4. Product workflow tests
cargo test -p brassclaw_product_workflow

# 5. Outbound/traces
cargo test -p brassclaw_outbound
cargo test -p brassclaw_reborn_traces

# 6. Architecture boundary checks
cargo test -p brassclaw_architecture

# 7. Full workspace clippy — zero warnings mandatory
cargo clippy --all --benches --tests --examples --all-features -- -D warnings

# 8. Full workspace build
cargo build --release --bin brassclaw

# 9. Integration tests (requires live Postgres)
cargo test --features integration
```

### 7.6 Manual smoke test with Meta Developer Console

Once deployed with a reachable HTTPS URL:

1. Register webhook URL in Meta App → Webhooks:
   - Callback URL: `https://<your-domain>/webhooks/whatsapp`
   - Verify token: the value you stored as `whatsapp_verify_token`
2. Meta sends GET challenge → expect 200 with challenge echoed.
3. Subscribe to `messages` field.
4. Send a WhatsApp DM from your personal number to the agent's business number.
5. Verify in the WebUI that a new thread appeared in the thread list.
6. Agent responds → verify the reply was delivered on WhatsApp.

---

## 8. Phase WA-H — Future work (out of scope for initial slice)

Document here but do not implement until the core slice is proven:

| Item | Notes |
|---|---|
| **Media attachments** | `type: image/audio/document` messages — download via `graph.facebook.com/<media_id>`, store through attachment pipeline, surface as `InboundAttachments`. |
| **Gate/auth prompts over WhatsApp** | Structured WhatsApp messages (buttons/lists) for approval flow. Requires `ExternalGatePush` capability. |
| **Typing indicator** | Meta Business API has no standard typing-indicator endpoint; investigate `markMessageRead` as a proxy signal. |
| **Per-user pairing / allowlist** | Currently any number that DMs the agent gets through. Add `pairing_upsert_request` / `pairing_resolve_identity` pattern from v1 channel if needed. |
| **Multi-installation** | Currently one phone number per agent. The DB schema supports multiple rows; the adapter registry needs a `phone_number_id` → `adapter` dispatch map. |
| **Webhook URL in Settings UI** | Display the exact URL to copy into Meta Console. Requires knowing the public hostname — inject at composition time from `BRASSCLAW_PUBLIC_URL` env var. |
| **Reconnect flow** | Token rotation: detect `401 FailedUnauthorized` → pause re-delivery → surface "credentials expired" in Settings UI. |

---

## 9. Phase WA-I — Dependency map

Every crate touched in phases WA-A through WA-G:

| Crate | Change type | Phase |
|---|---|---|
| `brassclaw_whatsapp_v2_adapter` | **NEW** | WA-A |
| `crates/brassclaw_pg/migrations` | Migration V062 | WA-B |
| `brassclaw_reborn_composition` | New module `whatsapp.rs`, new `whatsapp_installation_store.rs`, `webui_serve.rs` route wiring, `factory.rs` startup wiring | WA-C, WA-D |
| `brassclaw_product_workflow` | `StaticConnectableChannelsProductFacade` entry, new facade methods on `RebornServicesApi` | WA-D, WA-F |
| `brassclaw_reborn_traces` | `TraceChannel::WhatsApp` + `trace_channel_from_host_channel` | WA-E |
| `brassclaw_reborn_cli` | `TraceChannelArg::WhatsApp` + `From` impl + `Display` | WA-E |
| `brassclaw_webui_v2` | New handler file, new descriptors, router wiring | WA-F |
| `brassclaw_architecture` | Boundary test update | WA-G |
| `Cargo.toml` (workspace) | Add `brassclaw_whatsapp_v2_adapter` member | WA-A |

Crates that are **NOT touched** (confirms minimal scope):
`brassclaw_agent_loop`, `brassclaw_engine`, `brassclaw_turns`, `brassclaw_conversations`,
`brassclaw_host_runtime`, `brassclaw_secrets`, `brassclaw_outbound` (no schema changes),
`brassclaw_threads` (no schema changes), `brassclaw_llm`, `brassclaw_safety`.

The agent loop, the LLM call path, the recipe/v3 system, and the safety layer are
completely unchanged. WhatsApp is additive at the adapter boundary only.
