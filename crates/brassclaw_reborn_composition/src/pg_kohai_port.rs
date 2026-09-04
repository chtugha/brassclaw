//! Step C.5 slice 1c — composition-side [`KohaiPort`] impl (the FULL Kohai flow).
//!
//! [`PgKohaiPort`] is the composition-layer impl of the engine-side
//! [`brassclaw_engine::executor::kohai_port::KohaiPort`] trait. It owns the two
//! composition-side stores the Kohai handoff drives — the interceptor store
//! (`PgInterceptorStore` → `brassclaw_forensic_packets`) and the basic-prompt
//! prefix store (`PgBasicPromptStore` → `get_system_bundle`) — and runs the full
//! routing-state flow for `host.kohai_complete(prompt={chat_history, user_query,
//! prefix_placeholder})`:
//!
//! 1. Parse the prompt dict (`chat_history` + `user_query` + `prefix_placeholder`).
//! 2. Resolve the per-scope provider prefix via `get_system_bundle(user_id,
//!    project_id)` (infallible — falls back to the minimal base prompt).
//! 3. Build the final prompt messages (system prefix + chat_history + user
//!    query) + the [`CapturedPrompt`] for the forensic packet.
//! 4. `ForensicPacket::new(run_id, iteration, captured)` → `InterceptorStore::save`
//!    `[AwaitingKohai]`.
//! 5. (routing state — no Sempai wired) Build a [`HostManagedModelRequest`] (the
//!    `interactive_model` profile + system/history/user messages) →
//!    `HostManagedModelGateway::stream_model`. The Sempai audit step is the same
//!    path with a Sempai gateway + `SempaiProposalSink`; it lands behind this
//!    path and is gated on a Sempai sink being wired.
//! 6. `packet.with_kohai_response(text, usage)` → `InterceptorStore::save`
//!    `[Complete]`.
//! 7. Return [`KohaiAnswer`] (content + engine usage).
//!
//! The composition impl owns the provider [`HostManagedModelGateway`] (the real
//! working LLM) — the engine no longer threads an `LlmBackend` into the port
//! (the `LlmBackend` host path retires with the C.6 Kohai re-architecture).
//! Monty drives the provider call via the host fn, so "Monty, not the Rust
//! agent-loop, drives the LLM" holds.
//!
//! # Feature gate
//!
//! The DB-bound [`PgKohaiPort`] + its [`KohaiPort`] impl require the `postgres`
//! feature (the `PgBasicPromptStore` / `get_system_bundle` it reaches through are
//! `postgres`-gated). The pure mapping helpers + their unit tests touch only
//! always-available engine + interceptor types and compile/run under both configs,
//! mirroring `pg_composition_port.rs`. `#![allow(dead_code)]` covers the
//! unused-until-C.6-wiring window.

#![allow(dead_code)]
#![forbid(unsafe_code)]

use brassclaw_engine::executor::kohai_port::{
    KohaiAnswer, KohaiCallCtx, KohaiPort, KohaiPortError, KohaiUsage as EngineKohaiUsage,
};
use brassclaw_interceptor::packet::{
    CapturedPrompt, PromptSegment, TokenAccountingSnapshot,
};
use brassclaw_interceptor::{ForensicPacket, KohaiUsage as InterceptorKohaiUsage};
use brassclaw_loop_support::{
    HostManagedModelMessage, HostManagedModelMessageRole, HostManagedModelRequest,
    HostManagedModelResponse,
};
use brassclaw_turns::{
    LoopMessageRef, TurnId, TurnRunId,
    run_profile::{LoopModelUsage, ModelProfileId, ParentLoopOutput},
};

#[cfg(feature = "postgres")]
use std::{future::Future, pin::Pin, sync::Arc};
#[cfg(feature = "postgres")]
use brassclaw_interceptor::InterceptorStore;
#[cfg(feature = "postgres")]
use brassclaw_loop_support::HostManagedModelGateway;
#[cfg(feature = "postgres")]
use crate::pg_basic_prompt_store::{PgBasicPromptStore, get_system_bundle};

// ── pure mapping helpers (ungated; unit-tested under both configs) ───────────

/// 4-chars-per-token heuristic (matches the interceptor `packet.rs` convention).
fn estimate_tokens(s: &str) -> u32 {
    (s.len() as u32).div_ceil(4)
}

/// Pull a string field from the prompt dict (default empty when missing or
/// non-string).
fn prompt_string(prompt: &serde_json::Value, key: &str) -> String {
    prompt
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Parse `prompt["chat_history"]` into `(role, content)` pairs. Accepts a list
/// of `{"role","content"}` objects or `[role, content]` arrays; any other shape
/// is skipped.
fn prompt_chat_history(prompt: &serde_json::Value) -> Vec<(String, String)> {
    let Some(arr) = prompt.get("chat_history").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            if let Some(obj) = item.as_object() {
                let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                let content = obj.get("content").and_then(|v| v.as_str()).unwrap_or("");
                return Some((role.to_string(), content.to_string()));
            }
            if let Some(pair) = item.as_array() {
                let role = pair.first().and_then(|v| v.as_str()).unwrap_or("user");
                let content = pair.get(1).and_then(|v| v.as_str()).unwrap_or("");
                return Some((role.to_string(), content.to_string()));
            }
            None
        })
        .collect()
}

/// Build the [`CapturedPrompt`] for the forensic packet from the resolved system
/// prefix + the parsed chat history + the user query. The `messages` vec is the
/// exact prompt sent to the provider (system prefix + history + user query);
/// `segments` record the per-section token accounting; `capability_surface` is
/// empty (the Kohai handoff is a raw LLM call, not the full capability-bearing
/// agent-loop prompt).
fn build_captured_prompt(
    system_prefix: &str,
    chat_history: &[(String, String)],
    user_query: &str,
) -> CapturedPrompt {
    let mut messages: Vec<(String, String)> = Vec::with_capacity(2 + chat_history.len());
    messages.push(("system".to_string(), system_prefix.to_string()));
    for (role, content) in chat_history {
        messages.push((role.clone(), content.clone()));
    }
    if !user_query.is_empty() {
        messages.push(("user".to_string(), user_query.to_string()));
    }

    let mut segments = Vec::with_capacity(3);
    segments.push(PromptSegment {
        label: "system_prompt".to_string(),
        content: system_prefix.to_string(),
        estimated_tokens: estimate_tokens(system_prefix),
        inclusion_reason: "provider prefix (get_system_bundle)".to_string(),
        component_uuid: None,
    });
    let history_text = chat_history
        .iter()
        .map(|(r, c)| format!("{r}: {c}"))
        .collect::<Vec<_>>()
        .join("\n");
    if !history_text.is_empty() {
        segments.push(PromptSegment {
            label: "chat_history".to_string(),
            content: history_text.clone(),
            estimated_tokens: estimate_tokens(&history_text),
            inclusion_reason: "orchestrator-supplied chat history".to_string(),
            component_uuid: None,
        });
    }
    if !user_query.is_empty() {
        segments.push(PromptSegment {
            label: "user_query".to_string(),
            content: user_query.to_string(),
            estimated_tokens: estimate_tokens(user_query),
            inclusion_reason: "current turn user input".to_string(),
            component_uuid: None,
        });
    }

    let total_input_estimated = segments.iter().map(|s| s.estimated_tokens).sum();
    let message_count = messages.len() as u32;
    CapturedPrompt {
        messages,
        segments,
        token_accounting: TokenAccountingSnapshot {
            context_window_limit: 128_000,
            max_output_tokens: 8_192,
            total_input_estimated,
            message_count,
            kv_cache_optimised: false,
        },
        capability_surface_version: "v1".to_string(),
        visible_capability_count: 0,
    }
}

/// Map a captured `(role, content)` role string → the gateway message role.
/// Known roles map directly; anything else defaults to `User` (the Kohai
/// handoff chat history is orchestrator-supplied and typically user/assistant
/// turns only).
fn gateway_role_from_str(s: &str) -> HostManagedModelMessageRole {
    match s {
        "system" => HostManagedModelMessageRole::System,
        "assistant" => HostManagedModelMessageRole::Assistant,
        _ => HostManagedModelMessageRole::User,
    }
}

/// Build a per-message [`LoopMessageRef`] for the Kohai request. The opaque id
/// is `kohai-{run_str}-{label}` — `run_str` is a UUID display string (hex +
/// `-`) and `label` is a caller-supplied literal (`sys` / `h{n}` / `user`), so
/// the suffix is the `msg:` prefix + only `[A-Za-z0-9_.-]` chars (infallible).
/// Returns the validator's `Err` string untouched if an invalid `run_str`/label
/// is ever passed so the caller can map it to a [`KohaiPortError`].
fn kohai_message_ref(run_str: &str, label: &str) -> Result<LoopMessageRef, String> {
    LoopMessageRef::new(format!("msg:kohai-{run_str}-{label}"))
}

/// Build the [`HostManagedModelRequest`] from the captured prompt messages. The
/// `run_str` is the display string of the resolved [`TurnRunId`] (used only to
/// mint unique, valid `content_ref`s). Returns the request or a
/// [`KohaiPortError::InvalidPrompt`] when a `content_ref` fails validation
/// (only possible for a non-UUID `run_str`).
fn build_gateway_request(
    captured: &CapturedPrompt,
    model_profile_id: ModelProfileId,
    run_id: TurnRunId,
    turn_id: TurnId,
    run_str: &str,
) -> Result<HostManagedModelRequest, KohaiPortError> {
    let mut messages = Vec::with_capacity(captured.messages.len());
    for (idx, (role, content)) in captured.messages.iter().enumerate() {
        let label = match role.as_str() {
            "system" => "sys".to_string(),
            "user" if idx == captured.messages.len() - 1 => "user".to_string(),
            _ => format!("h{idx}"),
        };
        let content_ref = kohai_message_ref(run_str, &label).map_err(|e| {
            KohaiPortError::InvalidPrompt {
                reason: format!("message ref: {e}"),
            }
        })?;
        messages.push(HostManagedModelMessage {
            role: gateway_role_from_str(role),
            content: content.clone(),
            content_ref,
            tool_result_provider_call: None,
            tool_result_content: None,
        });
    }
    Ok(HostManagedModelRequest {
        model_profile_id,
        messages,
        surface_version: None,
        resolved_model_route: None,
        run_id,
        turn_id,
    })
}

/// Extract the answer text from a [`HostManagedModelResponse`]. Prefers the
/// `AssistantReply.content` (the sanitized final reply); falls back to the
/// joined `safe_text_deltas` when the output is not an assistant reply (e.g. a
/// capability-call shape, which the force-text Kohai path does not produce).
fn gateway_response_text(response: &HostManagedModelResponse) -> String {
    if let ParentLoopOutput::AssistantReply(reply) = &response.output
        && !reply.content.is_empty()
    {
        return reply.content.clone();
    }
    response.safe_text_deltas.join("")
}

/// Map the gateway [`LoopModelUsage`] (u32) → the interceptor packet
/// [`KohaiUsage`] (u32) for the forensic-packet close.
fn loop_usage_to_interceptor(usage: &LoopModelUsage) -> InterceptorKohaiUsage {
    InterceptorKohaiUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_input_tokens: usage.cache_read_input_tokens,
        cache_creation_input_tokens: usage.cache_creation_input_tokens,
    }
}

/// Map the gateway [`LoopModelUsage`] → the engine [`KohaiAnswer`] usage. The
/// gateway reports no USD cost (cost is accounted upstream by the budget
/// accountant from the same usage), so `cost_usd` is zero here.
fn loop_usage_to_engine(usage: &LoopModelUsage) -> EngineKohaiUsage {
    EngineKohaiUsage {
        input_tokens: u64::from(usage.input_tokens),
        output_tokens: u64::from(usage.output_tokens),
        cost_usd: 0.0,
    }
}

// ── DB-bound impl (postgres feature) ────────────────────────────────────────

/// Postgres-backed [`KohaiPort`] (the FULL Kohai flow). Constructed once at
/// runtime wiring time with the shared interceptor store + basic-prompt store +
/// the working provider gateway and plumbed into the engine `ExecutionLoop` via
/// `with_kohai_port`; until that C.6 wiring lands the engine passes `None` and
/// `host.kohai_complete` degrades gracefully (`{ok:false,
/// error:"kohai_unavailable"}`).
#[cfg(feature = "postgres")]
pub(crate) struct PgKohaiPort {
    interceptor_store: Arc<dyn InterceptorStore>,
    basic_prompt_store: Arc<PgBasicPromptStore>,
    kohai_gateway: Arc<dyn HostManagedModelGateway>,
    model_profile_id: ModelProfileId,
}

#[cfg(feature = "postgres")]
impl PgKohaiPort {
    /// Construct with the shared interceptor store + basic-prompt store + the
    /// working provider gateway (the "Kohai" LLM). Returns `Err` only if the
    /// `interactive_model` profile id is invalid (a compiled-in literal —
    /// infallible in practice).
    pub(crate) fn new(
        interceptor_store: Arc<dyn InterceptorStore>,
        basic_prompt_store: Arc<PgBasicPromptStore>,
        kohai_gateway: Arc<dyn HostManagedModelGateway>,
    ) -> Result<Self, String> {
        let model_profile_id = ModelProfileId::new("interactive_model")?;
        Ok(Self {
            interceptor_store,
            basic_prompt_store,
            kohai_gateway,
            model_profile_id,
        })
    }

    /// The full Kohai flow (steps 1-7 above). Takes the stores + gateway by
    /// reference so the trait impl can clone the call args into owned data and
    /// drive the future off the cloned `Arc`s.
    async fn complete_with_stores(
        interceptor_store: &Arc<dyn InterceptorStore>,
        basic_prompt_store: &PgBasicPromptStore,
        kohai_gateway: &dyn HostManagedModelGateway,
        model_profile_id: ModelProfileId,
        prompt: serde_json::Value,
        ctx: KohaiCallCtx,
    ) -> Result<KohaiAnswer, KohaiPortError> {
        // 1. Parse the prompt dict.
        let chat_history = prompt_chat_history(&prompt);
        let user_query = prompt_string(&prompt, "user_query");
        // `prefix_placeholder` is a Monty-side marker; the actual prefix is
        // resolved per-scope from the bundle (step 2). Recorded for contract
        // fidelity, not used to locate a swap site.
        let _prefix_placeholder = prompt_string(&prompt, "prefix_placeholder");

        // 2. Resolve the provider prefix for this scope (infallible — falls
        //    back to the minimal base prompt).
        let system_prefix =
            get_system_bundle(basic_prompt_store, &ctx.user_id, &ctx.project_id).await;

        // 3. Build the final prompt messages + forensic capture.
        let captured = build_captured_prompt(&system_prefix, &chat_history, &user_query);

        // 4. Capture [AwaitingKohai] → save.
        let packet = ForensicPacket::new(ctx.run_id.clone(), ctx.iteration, captured);
        interceptor_store
            .save(&packet)
            .await
            .map_err(|e| KohaiPortError::StoreFailed {
                reason: e.to_string(),
            })?;

        // 5. (routing — no Sempai wired) Build the gateway request + call. The
        //    run id is the engine thread id (a UUID) when parseable, else a
        //    fresh id; the turn id is fresh per Kohai call.
        let run_id = TurnRunId::parse(&ctx.run_id).unwrap_or_else(|_| TurnRunId::new());
        let run_str = run_id.to_string();
        let turn_id = TurnId::new();
        let request = build_gateway_request(
            &packet.prompt,
            model_profile_id,
            run_id,
            turn_id,
            &run_str,
        )?;
        let response = kohai_gateway
            .stream_model(request)
            .await
            .map_err(|e| KohaiPortError::LlmFailed {
                reason: e.to_string(),
            })?;
        let answer_text = gateway_response_text(&response);

        // 6. Close [Complete] → save.
        let (engine_usage, interceptor_usage) = match response.usage {
            Some(usage) => {
                let engine = loop_usage_to_engine(&usage);
                let interceptor = loop_usage_to_interceptor(&usage);
                (engine, Some(interceptor))
            }
            None => (EngineKohaiUsage::default(), None),
        };
        let packet = packet.with_kohai_response(answer_text.clone(), interceptor_usage);
        interceptor_store
            .save(&packet)
            .await
            .map_err(|e| KohaiPortError::StoreFailed {
                reason: e.to_string(),
            })?;

        // 7. Return the engine answer.
        Ok(KohaiAnswer {
            content: answer_text,
            usage: engine_usage,
        })
    }
}

#[cfg(feature = "postgres")]
impl KohaiPort for PgKohaiPort {
    fn complete(
        &self,
        prompt: serde_json::Value,
        ctx: KohaiCallCtx,
    ) -> Pin<Box<dyn Future<Output = Result<KohaiAnswer, KohaiPortError>> + Send + 'static>> {
        // Clone the Arc stores + gateway + the owned prompt/ctx into the boxed
        // future so it borrows nothing (the impl owns the provider gateway — no
        // borrowed `LlmBackend`), matching the `ComponentPort::compose`
        // `'static` precedent.
        let interceptor_store = self.interceptor_store.clone();
        let basic_prompt_store = self.basic_prompt_store.clone();
        let kohai_gateway = self.kohai_gateway.clone();
        let model_profile_id = self.model_profile_id.clone();
        Box::pin(async move {
            Self::complete_with_stores(
                &interceptor_store,
                basic_prompt_store.as_ref(),
                kohai_gateway.as_ref(),
                model_profile_id,
                prompt,
                ctx,
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_ceil_divides_by_four() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("You are an assistant."), 6);
    }

    #[test]
    fn prompt_string_returns_field_or_empty() {
        let p = serde_json::json!({"user_query": "hi"});
        assert_eq!(prompt_string(&p, "user_query"), "hi");
        assert_eq!(prompt_string(&p, "missing"), "");
        let p2 = serde_json::json!({"user_query": 7});
        assert_eq!(prompt_string(&p2, "user_query"), "");
    }

    #[test]
    fn prompt_chat_history_parses_object_and_array_forms() {
        let p = serde_json::json!({
            "chat_history": [
                {"role": "user", "content": "hello"},
                ["assistant", "hi there"],
                "junk-scalar"
            ]
        });
        let hist = prompt_chat_history(&p);
        assert_eq!(
            hist,
            vec![
                ("user".to_string(), "hello".to_string()),
                ("assistant".to_string(), "hi there".to_string()),
            ]
        );
    }

    #[test]
    fn prompt_chat_history_missing_returns_empty() {
        let p = serde_json::json!({"user_query": "hi"});
        assert!(prompt_chat_history(&p).is_empty());
    }

    #[test]
    fn build_captured_prompt_assembles_system_history_user() {
        let hist = vec![
            ("user".to_string(), "earlier".to_string()),
            ("assistant".to_string(), "reply".to_string()),
        ];
        let cap = build_captured_prompt("SYS", &hist, "current q");
        assert_eq!(cap.messages[0], ("system".to_string(), "SYS".to_string()));
        assert_eq!(cap.messages[1], ("user".to_string(), "earlier".to_string()));
        assert_eq!(
            cap.messages[2],
            ("assistant".to_string(), "reply".to_string())
        );
        assert_eq!(cap.messages[3], ("user".to_string(), "current q".to_string()));
        assert_eq!(cap.token_accounting.message_count, 4);
        assert!(cap.token_accounting.total_input_estimated > 0);
        assert_eq!(cap.segments.len(), 3);
        assert_eq!(cap.segments[0].label, "system_prompt");
        assert_eq!(cap.segments[2].label, "user_query");
        assert_eq!(cap.visible_capability_count, 0);
    }

    #[test]
    fn build_captured_prompt_omits_empty_user_query() {
        let cap = build_captured_prompt("SYS", &[], "");
        assert_eq!(
            cap.messages,
            vec![("system".to_string(), "SYS".to_string())]
        );
        assert_eq!(cap.token_accounting.message_count, 1);
        assert_eq!(cap.segments.len(), 1);
    }

    #[test]
    fn gateway_role_from_str_maps_known_and_defaults_to_user() {
        assert_eq!(
            gateway_role_from_str("system"),
            HostManagedModelMessageRole::System
        );
        assert_eq!(
            gateway_role_from_str("assistant"),
            HostManagedModelMessageRole::Assistant
        );
        assert_eq!(
            gateway_role_from_str("user"),
            HostManagedModelMessageRole::User
        );
        assert_eq!(
            gateway_role_from_str("unknown"),
            HostManagedModelMessageRole::User
        );
    }

    #[test]
    fn kohai_message_ref_accepts_uuid_run_str() {
        let run_str = "01234567-89ab-cdef-0123-456789abcdef";
        let r = kohai_message_ref(run_str, "sys").expect("valid ref");
        assert_eq!(r.as_str(), format!("msg:kohai-{run_str}-sys"));
        // A label containing an illegal `:` suffix is rejected (the validator
        // forbids `:` after the `msg:` prefix colon).
        assert!(kohai_message_ref(run_str, "b:ad").is_err());
    }

    #[test]
    fn build_gateway_request_assembles_messages_with_refs() {
        let hist = vec![
            ("user".to_string(), "earlier".to_string()),
            ("assistant".to_string(), "reply".to_string()),
        ];
        let captured = build_captured_prompt("SYS", &hist, "current q");
        let run_id = TurnRunId::new();
        let run_str = run_id.to_string();
        let profile = ModelProfileId::new("interactive_model").expect("profile id");
        let request = build_gateway_request(
            &captured,
            profile.clone(),
            run_id,
            TurnId::new(),
            &run_str,
        )
        .expect("request builds");
        assert_eq!(request.model_profile_id, profile);
        assert_eq!(request.messages.len(), 4);
        assert_eq!(request.messages[0].role, HostManagedModelMessageRole::System);
        assert_eq!(request.messages[0].content, "SYS");
        assert_eq!(request.messages[3].role, HostManagedModelMessageRole::User);
        assert_eq!(request.messages[3].content, "current q");
        assert!(request.messages[0].content_ref.as_str().starts_with("msg:kohai-"));
    }

    #[test]
    fn build_gateway_request_rejects_invalid_run_str() {
        let captured = build_captured_prompt("SYS", &[], "");
        let run_str = "not a uuid"; // contains a space → illegal opaque-id char
        let err = build_gateway_request(
            &captured,
            ModelProfileId::new("interactive_model").expect("profile id"),
            TurnRunId::new(),
            TurnId::new(),
            run_str,
        )
        .expect_err("invalid run_str rejected");
        assert!(matches!(err, KohaiPortError::InvalidPrompt { .. }));
    }

    #[test]
    fn gateway_response_text_prefers_assistant_reply() {
        let resp = HostManagedModelResponse::assistant_reply("hello");
        assert_eq!(gateway_response_text(&resp), "hello");
    }

    #[test]
    fn gateway_response_text_falls_back_to_deltas_for_non_reply() {
        let resp = HostManagedModelResponse {
            safe_text_deltas: vec!["d1".to_string(), "d2".to_string()],
            safe_reasoning_deltas: Vec::new(),
            output: ParentLoopOutput::CapabilityCalls(Vec::new()),
            usage: None,
        };
        assert_eq!(gateway_response_text(&resp), "d1d2");
    }

    #[test]
    fn loop_usage_maps_to_interceptor_and_engine() {
        let usage = LoopModelUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 5,
            cache_creation_input_tokens: 3,
        };
        let ic = loop_usage_to_interceptor(&usage);
        assert_eq!(ic.input_tokens, 100);
        assert_eq!(ic.output_tokens, 50);
        assert_eq!(ic.cache_read_input_tokens, 5);
        assert_eq!(ic.cache_creation_input_tokens, 3);
        let en = loop_usage_to_engine(&usage);
        assert_eq!(en.input_tokens, 100);
        assert_eq!(en.output_tokens, 50);
        assert_eq!(en.cost_usd, 0.0);
    }
}
