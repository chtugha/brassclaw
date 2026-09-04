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
//! 5. (routing state — no Sempai wired) Build `ThreadMessage`s →
//!    `LlmBackend::complete` (`force_text`). The Sempai audit step is the same
//!    path with a Sempai LLM + `SempaiProposalSink`; it lands behind this path
//!    and is gated on a Sempai sink being wired.
//! 6. `packet.with_kohai_response(text, usage)` → `InterceptorStore::save`
//!    `[Complete]`.
//! 7. Return [`KohaiAnswer`] (content + engine usage).
//!
//! The engine's `LlmBackend` is passed INTO the port call (the composition impl
//! does not own an LLM backend) — Monty drives the provider call via the host fn,
//! so "Monty, not the Rust agent-loop, drives the LLM" holds. `LlmBackend` reaches
//! the provider over http internally (the `first_party_tools/http` route in the
//! seed is the conceptual route).
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
use brassclaw_engine::traits::llm::{LlmBackend, LlmCallConfig};
use brassclaw_engine::types::message::{MessageRole, ThreadMessage};
use brassclaw_engine::types::step::{LlmResponse, TokenUsage};
use brassclaw_interceptor::packet::{
    CapturedPrompt, PromptSegment, TokenAccountingSnapshot,
};
use brassclaw_interceptor::{ForensicPacket, KohaiUsage as InterceptorKohaiUsage};

#[cfg(feature = "postgres")]
use std::{future::Future, pin::Pin, sync::Arc};
#[cfg(feature = "postgres")]
use brassclaw_interceptor::InterceptorStore;
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
        });
    }
    if !user_query.is_empty() {
        segments.push(PromptSegment {
            label: "user_query".to_string(),
            content: user_query.to_string(),
            estimated_tokens: estimate_tokens(user_query),
            inclusion_reason: "current turn user input".to_string(),
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

/// Map a prompt-dict role string → [`MessageRole`]. Known roles map directly;
/// anything else defaults to `User` (the Kohai handoff chat history is
/// orchestrator-supplied and typically user/assistant turns only).
fn role_from_str(s: &str) -> MessageRole {
    match s {
        "system" => MessageRole::System,
        "assistant" => MessageRole::Assistant,
        _ => MessageRole::User,
    }
}

/// Build the [`ThreadMessage`] sequence for the `LlmBackend::complete` call from
/// the captured `(role, content)` pairs.
fn to_thread_messages(messages: &[(String, String)]) -> Vec<ThreadMessage> {
    messages
        .iter()
        .map(|(role, content)| match role_from_str(role) {
            MessageRole::System => ThreadMessage::system(content.clone()),
            MessageRole::Assistant => ThreadMessage::assistant(content.clone()),
            MessageRole::User => ThreadMessage::user(content.clone()),
            MessageRole::ActionResult => ThreadMessage::user(content.clone()),
        })
        .collect()
}

/// Map the engine [`TokenUsage`] (u64) → the interceptor packet [`KohaiUsage`]
/// (u32) for the forensic-packet close.
fn token_usage_to_interceptor(usage: &TokenUsage) -> InterceptorKohaiUsage {
    InterceptorKohaiUsage {
        input_tokens: u32::try_from(usage.input_tokens).unwrap_or(u32::MAX),
        output_tokens: u32::try_from(usage.output_tokens).unwrap_or(u32::MAX),
        cache_read_input_tokens: u32::try_from(usage.cache_read_tokens).unwrap_or(u32::MAX),
        cache_creation_input_tokens: u32::try_from(usage.cache_write_tokens).unwrap_or(u32::MAX),
    }
}

/// Map the engine [`TokenUsage`] → the engine [`KohaiAnswer`] usage.
fn token_usage_to_engine(usage: &TokenUsage) -> EngineKohaiUsage {
    EngineKohaiUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cost_usd: usage.cost_usd,
    }
}

/// Extract the answer text from an [`LlmResponse`]. `Text` → its string;
/// `ActionCalls` / `Code` → their optional reasoning `content` (empty when
/// absent). With `force_text = true` the provider returns `Text`.
fn llm_response_text(response: &LlmResponse) -> String {
    match response {
        LlmResponse::Text(s) => s.clone(),
        LlmResponse::ActionCalls { content, .. } => content.clone().unwrap_or_default(),
        LlmResponse::Code { content, .. } => content.clone().unwrap_or_default(),
    }
}

// ── DB-bound impl (postgres feature) ────────────────────────────────────────

/// Postgres-backed [`KohaiPort`] (the FULL Kohai flow). Constructed once at
/// runtime wiring time with the shared interceptor store + basic-prompt store
/// and plumbed into the engine `ExecutionLoop` via `with_kohai_port`; until that
/// C.6 wiring lands the engine passes `None` and `host.kohai_complete` degrades
/// gracefully (`{ok:false, error:"kohai_unavailable"}`).
#[cfg(feature = "postgres")]
pub(crate) struct PgKohaiPort {
    interceptor_store: Arc<dyn InterceptorStore>,
    basic_prompt_store: Arc<PgBasicPromptStore>,
}

#[cfg(feature = "postgres")]
impl PgKohaiPort {
    pub(crate) fn new(
        interceptor_store: Arc<dyn InterceptorStore>,
        basic_prompt_store: Arc<PgBasicPromptStore>,
    ) -> Self {
        Self {
            interceptor_store,
            basic_prompt_store,
        }
    }

    /// The full Kohai flow (steps 1-7 above). Takes the stores by reference so
    /// the trait impl can clone the call args into owned data and drive the
    /// future off the cloned `Arc`s.
    async fn complete_with_stores(
        interceptor_store: &Arc<dyn InterceptorStore>,
        basic_prompt_store: &PgBasicPromptStore,
        prompt: serde_json::Value,
        ctx: KohaiCallCtx,
        llm: &dyn LlmBackend,
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

        // 5. (routing — no Sempai wired) Build the LLM messages + call.
        let messages = to_thread_messages(&packet.prompt.messages);
        let config = LlmCallConfig {
            force_text: true,
            ..Default::default()
        };
        let output = llm
            .complete(&messages, &[], &config)
            .await
            .map_err(|e| KohaiPortError::LlmFailed {
                reason: e.to_string(),
            })?;
        let answer_text = llm_response_text(&output.response);

        // 6. Close [Complete] → save.
        let packet = packet.with_kohai_response(
            answer_text.clone(),
            Some(token_usage_to_interceptor(&output.usage)),
        );
        interceptor_store
            .save(&packet)
            .await
            .map_err(|e| KohaiPortError::StoreFailed {
                reason: e.to_string(),
            })?;

        // 7. Return the engine answer.
        Ok(KohaiAnswer {
            content: answer_text,
            usage: token_usage_to_engine(&output.usage),
        })
    }
}

#[cfg(feature = "postgres")]
impl KohaiPort for PgKohaiPort {
    fn complete<'a>(
        &'a self,
        prompt: serde_json::Value,
        ctx: KohaiCallCtx,
        llm: &'a dyn LlmBackend,
    ) -> Pin<Box<dyn Future<Output = Result<KohaiAnswer, KohaiPortError>> + Send + 'a>> {
        // Clone the Arc stores into owned data so the boxed future borrows only
        // `llm` for `'a` (the trait's `+ 'a` return captures the borrowed
        // LlmBackend, which the engine handler holds alive across the `.await`).
        let interceptor_store = self.interceptor_store.clone();
        let basic_prompt_store = self.basic_prompt_store.clone();
        Box::pin(async move {
            Self::complete_with_stores(
                &interceptor_store,
                basic_prompt_store.as_ref(),
                prompt,
                ctx,
                llm,
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::types::step::TokenUsage;

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
    fn role_from_str_maps_known_and_defaults_to_user() {
        assert_eq!(role_from_str("system"), MessageRole::System);
        assert_eq!(role_from_str("assistant"), MessageRole::Assistant);
        assert_eq!(role_from_str("user"), MessageRole::User);
        assert_eq!(role_from_str("unknown"), MessageRole::User);
    }

    #[test]
    fn to_thread_messages_builds_correct_roles() {
        let msgs = vec![
            ("system".to_string(), "s".to_string()),
            ("user".to_string(), "u".to_string()),
            ("assistant".to_string(), "a".to_string()),
        ];
        let tm = to_thread_messages(&msgs);
        assert_eq!(tm.len(), 3);
        assert_eq!(tm[0].role, MessageRole::System);
        assert_eq!(tm[0].content, "s");
        assert_eq!(tm[1].role, MessageRole::User);
        assert_eq!(tm[2].role, MessageRole::Assistant);
    }

    #[test]
    fn token_usage_maps_to_interceptor_and_engine() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 5,
            cache_write_tokens: 3,
            cost_usd: 0.01,
        };
        let ic = token_usage_to_interceptor(&usage);
        assert_eq!(ic.input_tokens, 100);
        assert_eq!(ic.output_tokens, 50);
        assert_eq!(ic.cache_read_input_tokens, 5);
        assert_eq!(ic.cache_creation_input_tokens, 3);
        let en = token_usage_to_engine(&usage);
        assert_eq!(en.input_tokens, 100);
        assert_eq!(en.output_tokens, 50);
        assert_eq!(en.cost_usd, 0.01);
    }

    #[test]
    fn llm_response_text_extracts_text_and_fallbacks() {
        assert_eq!(llm_response_text(&LlmResponse::Text("hi".into())), "hi");
        assert_eq!(
            llm_response_text(&LlmResponse::ActionCalls {
                calls: vec![],
                content: Some("reason".into())
            }),
            "reason"
        );
        assert_eq!(
            llm_response_text(&LlmResponse::ActionCalls {
                calls: vec![],
                content: None
            }),
            ""
        );
    }
}
