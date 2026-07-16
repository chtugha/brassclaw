//! `ForensicPacket` — the core data type captured by the interceptor.
//!
//! Each turn through the agent loop produces exactly one `ForensicPacket`.
//! The packet is created after `PromptStage` completes and closed (with
//! Kohai response + optional Sempai review) after `ModelStage` completes.
//!
//! # Lifecycle
//!
//! ```text
//! PromptStage completes
//!   → ForensicPacket::from_prompt()           [status: AwaitingKohai]
//!   → InterceptorStore::save()
//!   → (if rerouting) Sempai audit prompt sent
//!   → (if rerouting) SempaiResponseParts received
//! ModelStage completes (Kohai response)
//!   → ForensicPacket::with_kohai_response()   [status: Complete]
//!   → (if rerouting) with_sempai_review()     [status: SempaiReviewed]
//!   → InterceptorStore::save()
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identifier for a `ForensicPacket` within the interceptor store.
/// Carried alongside the prompt as it travels to the Kohai provider so the
/// response can be correlated back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PacketId(pub String);

impl PacketId {
    /// Mint a new random `PacketId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for PacketId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PacketId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Lifecycle status of a `ForensicPacket`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketStatus {
    /// Prompt captured, Kohai response not yet received.
    AwaitingKohai,
    /// Kohai response received; no Sempai review was performed (routing state).
    Complete,
    /// Sempai reviewed the prompt and optionally adjusted it before Kohai call.
    SempaiReviewed,
}

/// One captured prompt segment — corresponds to a logical section of the
/// assembled prompt (system instructions, skill context, recipe hints,
/// conversation history, capability surface, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSegment {
    /// Human-readable label identifying the segment (e.g. `"system_prompt"`,
    /// `"skill:ibm_bob_people"`, `"recipe_hint:deploy-workflow"`).
    pub label: String,
    /// The text content of this segment as it appeared in the final prompt.
    pub content: String,
    /// Estimated token count for this segment (4-chars-per-token heuristic,
    /// matching `brassclaw_agent_loop::token_budget::estimate_tokens`).
    pub estimated_tokens: u32,
    /// Why this segment was included — a short description of the decision
    /// path (e.g. `"skill activated: score=45 keyword=ibm"`,
    /// `"recipe matched: wilson=0.82 tier=mature"`).
    pub inclusion_reason: String,
}

/// Full budget accounting snapshot taken at prompt-assembly time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAccountingSnapshot {
    /// Maximum context window the Kohai model accepts (tokens).
    pub context_window_limit: u32,
    /// Maximum output tokens allowed for this turn.
    pub max_output_tokens: u32,
    /// Total input tokens in the assembled prompt (estimated).
    pub total_input_estimated: u32,
    /// Number of messages in the assembled prompt.
    pub message_count: u32,
    /// Whether KV-cache-optimised prompt ordering was applied.
    pub kv_cache_optimised: bool,
}

/// The assembled prompt sent to the Kohai provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedPrompt {
    /// All messages in the final prompt (role + content-ref text).
    /// Each element is `(role, content_text)`.
    pub messages: Vec<(String, String)>,
    /// Logical segments that were assembled to build the prompt, with
    /// per-segment token accounting and inclusion decision metadata.
    pub segments: Vec<PromptSegment>,
    /// Token budget snapshot at assembly time.
    pub token_accounting: TokenAccountingSnapshot,
    /// Capability surface version that was visible during prompt assembly.
    pub capability_surface_version: String,
    /// Number of capabilities visible to the model on this turn.
    pub visible_capability_count: u32,
}

/// Sempai review outcome — returned by the Sempai provider and stored
/// alongside the original `ForensicPacket`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SempaiReviewOutcome {
    /// The adjusted Kohai prompt (all messages, as adjusted by Sempai).
    /// This is the prompt that is actually sent to the Kohai model.
    pub adjusted_messages: Vec<(String, String)>,
    /// Sempai's summary of the prompt composition analysis — what it
    /// observed, what it adjusted, and why.
    pub composition_summary: String,
    /// Recipe/ToolSkill updates proposed by the Sempai, as raw JSON
    /// payloads forwarded to the validation queue.
    pub proposed_recipe_updates: Vec<serde_json::Value>,
    /// Optional agent-settings adjustments proposed by the Sempai
    /// (forwarded to the settings service for application).
    pub settings_adjustments: Vec<serde_json::Value>,
}

/// The central telemetry record for one agent-loop turn.
///
/// A `ForensicPacket` is created after `PromptStage` and completed after
/// `ModelStage`.  In routing state it records everything for offline
/// analysis; in rerouting state Sempai reviews the prompt before it
/// reaches the Kohai model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicPacket {
    /// Stable identifier for this packet — carried alongside the Kohai
    /// call so the response can be correlated back.
    pub id: PacketId,
    /// Lifecycle status.
    pub status: PacketStatus,
    /// The turn/run identifiers from the host.
    pub run_id: String,
    pub iteration: u32,
    /// Timestamp when the prompt was captured (after PromptStage).
    pub captured_at: DateTime<Utc>,
    /// The assembled prompt and its structural breakdown.
    pub prompt: CapturedPrompt,
    /// Raw Kohai response text (set after ModelStage completes).
    /// `None` while status is `AwaitingKohai`.
    pub kohai_response: Option<String>,
    /// Actual token usage as reported by the Kohai provider (set after
    /// ModelStage completes).
    pub kohai_usage: Option<KohaiUsage>,
    /// Sempai review outcome — present only when status is `SempaiReviewed`.
    pub sempai_review: Option<SempaiReviewOutcome>,
    /// Timestamp when the Kohai response was received.
    pub completed_at: Option<DateTime<Utc>>,
}

/// Token usage as reported by the Kohai provider for this turn.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KohaiUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_input_tokens: u32,
    pub cache_creation_input_tokens: u32,
}

impl ForensicPacket {
    /// Create a new packet from a captured prompt.  Status is
    /// `AwaitingKohai` — the Kohai response has not yet been received.
    pub fn new(
        run_id: impl Into<String>,
        iteration: u32,
        prompt: CapturedPrompt,
    ) -> Self {
        Self {
            id: PacketId::new(),
            status: PacketStatus::AwaitingKohai,
            run_id: run_id.into(),
            iteration,
            captured_at: Utc::now(),
            prompt,
            kohai_response: None,
            kohai_usage: None,
            sempai_review: None,
            completed_at: None,
        }
    }

    /// Attach the Kohai response and mark the packet as `Complete`
    /// (routing state — no Sempai review performed).
    pub fn with_kohai_response(
        mut self,
        response_text: impl Into<String>,
        usage: Option<KohaiUsage>,
    ) -> Self {
        self.kohai_response = Some(response_text.into());
        self.kohai_usage = usage;
        self.status = PacketStatus::Complete;
        self.completed_at = Some(Utc::now());
        self
    }

    /// Attach both the Kohai response and the Sempai review outcome,
    /// marking the packet as `SempaiReviewed`.
    pub fn with_sempai_review(
        mut self,
        response_text: impl Into<String>,
        usage: Option<KohaiUsage>,
        review: SempaiReviewOutcome,
    ) -> Self {
        self.kohai_response = Some(response_text.into());
        self.kohai_usage = usage;
        self.sempai_review = Some(review);
        self.status = PacketStatus::SempaiReviewed;
        self.completed_at = Some(Utc::now());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_prompt() -> CapturedPrompt {
        CapturedPrompt {
            messages: vec![
                ("system".to_string(), "You are an assistant.".to_string()),
                ("user".to_string(), "Help me deploy".to_string()),
            ],
            segments: vec![PromptSegment {
                label: "system_prompt".to_string(),
                content: "You are an assistant.".to_string(),
                estimated_tokens: 6,
                inclusion_reason: "always included".to_string(),
            }],
            token_accounting: TokenAccountingSnapshot {
                context_window_limit: 128_000,
                max_output_tokens: 8_192,
                total_input_estimated: 6,
                message_count: 2,
                kv_cache_optimised: true,
            },
            capability_surface_version: "v1".to_string(),
            visible_capability_count: 12,
        }
    }

    #[test]
    fn packet_id_is_unique() {
        let a = PacketId::new();
        let b = PacketId::new();
        assert_ne!(a.0, b.0);
    }

    #[test]
    fn new_packet_awaiting_kohai() {
        let packet = ForensicPacket::new("run-1", 0, make_prompt());
        assert_eq!(packet.status, PacketStatus::AwaitingKohai);
        assert!(packet.kohai_response.is_none());
        assert!(packet.completed_at.is_none());
    }

    #[test]
    fn with_kohai_response_marks_complete() {
        let packet = ForensicPacket::new("run-1", 0, make_prompt())
            .with_kohai_response("Sure, deploying now.", None);
        assert_eq!(packet.status, PacketStatus::Complete);
        assert_eq!(packet.kohai_response.as_deref(), Some("Sure, deploying now."));
        assert!(packet.completed_at.is_some());
        assert!(packet.sempai_review.is_none());
    }

    #[test]
    fn with_sempai_review_marks_reviewed() {
        let review = SempaiReviewOutcome {
            adjusted_messages: vec![("system".to_string(), "Adjusted system prompt".to_string())],
            composition_summary: "Improved token ordering for KV cache utilisation.".to_string(),
            proposed_recipe_updates: vec![],
            settings_adjustments: vec![],
        };
        let packet = ForensicPacket::new("run-1", 0, make_prompt())
            .with_sempai_review("OK", None, review);
        assert_eq!(packet.status, PacketStatus::SempaiReviewed);
        assert!(packet.sempai_review.is_some());
    }

    #[test]
    fn packet_id_display_matches_inner() {
        let id = PacketId::new();
        assert_eq!(id.to_string(), id.0);
    }
}
