use std::collections::HashMap;
use std::sync::Mutex;

use brassclaw_turns::{AcceptedMessageRef, TurnScope};
use thiserror::Error;

/// Maximum number of first-party skills selected for one turn by default.
pub const DEFAULT_MAX_ACTIVE_SKILLS: usize = 4;

/// Maximum estimated skill prompt tokens selected for one turn by default.
pub const DEFAULT_MAX_SKILL_CONTEXT_TOKENS: usize = 4000;

/// Typed request produced by first-party skill activation selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillActivationRequest {
    pub name: String,
    pub mode: SkillActivationMode,
}

/// Why a skill activation request was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillActivationMode {
    ExplicitMention,
    ActivationCriteria,
    ModelSelected,
}

/// Selector limits for conversation-driven first-party skill activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillActivationSelectorConfig {
    pub max_active_skills: usize,
    pub max_context_tokens: usize,
    pub selection_mode: SkillActivationSelectionMode,
    pub regex_activation_enabled: bool,
}

/// How recorded user messages are allowed to activate skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillActivationSelectionMode {
    ExplicitAndCriteria,
    ExplicitOnly,
}

impl Default for SkillActivationSelectorConfig {
    fn default() -> Self {
        Self {
            max_active_skills: DEFAULT_MAX_ACTIVE_SKILLS,
            max_context_tokens: DEFAULT_MAX_SKILL_CONTEXT_TOKENS,
            selection_mode: SkillActivationSelectionMode::ExplicitAndCriteria,
            regex_activation_enabled: true,
        }
    }
}

/// Result of selecting skill activations from one user message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillActivationSelection {
    pub activations: Vec<SkillActivationRequest>,
    pub rewritten_message: String,
    pub feedback: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillActivationObservedEvent {
    pub run_context: brassclaw_turns::run_profile::LoopRunContext,
    pub activations: Vec<SkillActivationRequest>,
    pub feedback: Vec<String>,
}

pub trait SkillActivationObserver: std::fmt::Debug + Send + Sync {
    fn observe_skill_activation(&self, event: SkillActivationObservedEvent);
}

/// Fully resolved activation output for one user message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillActivationPlan {
    pub selection: SkillActivationSelection,
}

impl SkillActivationPlan {
    pub fn empty(selection: SkillActivationSelection) -> Self {
        Self { selection }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SkillActivationSelectionError {
    #[error("skill activation source unavailable")]
    SourceUnavailable,
    #[error("skill activation parse failed")]
    ParseFailed,
    #[error("skill activation visibility data missing")]
    VisibilityDataMissing,
    #[error("skill activation context budget exceeded")]
    ContextBudgetExceeded,
    #[error("skill activation internal error")]
    Internal,
}

/// Message-text recorder for a Reborn loop turn scope.
///
/// In v3, skills are DB components injected via the `PgBasicPromptStore` prefix
/// bundle (Phase K.1). This struct is the thin store that maps a turn's
/// `(scope, accepted_message_ref)` to the raw user text so
/// `SkillActivationMessageTextResolver::resolve_message_text` can recover it
/// for intent matching in `InputStage`.
///
/// The VFS-based SKILL.md loading, candidate selection, and execution-adapter
/// paths were removed in Phase P.1 Step C (subplan_step8_of_plan_skill_context_removal.md).
#[derive(Debug)]
pub struct SelectableSkillContextSource {
    messages_by_run: Mutex<HashMap<SkillActivationMessageKey, String>>,
}

impl SelectableSkillContextSource {
    pub fn new() -> Self {
        Self {
            messages_by_run: Mutex::new(HashMap::new()),
        }
    }

    pub fn record_user_message(
        &self,
        scope: TurnScope,
        accepted_message_ref: AcceptedMessageRef,
        message: impl Into<String>,
    ) -> Result<(), SkillActivationSelectionError> {
        self.messages_by_run
            .lock()
            .map_err(|_| SkillActivationSelectionError::Internal)?
            .insert(
                SkillActivationMessageKey::new(scope, accepted_message_ref),
                message.into(),
            );
        Ok(())
    }

    pub fn clear_accepted_message(
        &self,
        scope: &TurnScope,
        accepted_message_ref: &AcceptedMessageRef,
    ) -> Result<(), SkillActivationSelectionError> {
        self.messages_by_run
            .lock()
            .map_err(|_| SkillActivationSelectionError::Internal)?
            .remove(&SkillActivationMessageKey::new(
                scope.clone(),
                accepted_message_ref.clone(),
            ));
        Ok(())
    }

    /// Non-consuming read of the raw accepted-message text recorded for
    /// `(scope, accepted_message_ref)` (v3 plan §H3). Returns `None` when no
    /// message is recorded (already taken by the activation path or never
    /// written). Does NOT remove the entry — so the text remains available for
    /// intent-driven retrieval across the turn.
    ///
    /// Returns the **raw** text, NOT the sanitized `safe_summary`, so intent
    /// matching is not corrupted by `[redacted]` placeholders. Backs the
    /// composition `MessageTextResolver` impl that the production host wires
    /// via `with_message_text_resolver`.
    pub fn peek_message_text(
        &self,
        scope: &TurnScope,
        accepted_message_ref: &AcceptedMessageRef,
    ) -> Result<Option<String>, SkillActivationSelectionError> {
        Ok(self
            .messages_by_run
            .lock()
            .map_err(|_| SkillActivationSelectionError::Internal)?
            .get(&SkillActivationMessageKey::new(
                scope.clone(),
                accepted_message_ref.clone(),
            ))
            .cloned())
    }

    #[cfg(test)]
    fn take_message_for_run(
        &self,
        scope: &TurnScope,
        accepted_message_ref: &AcceptedMessageRef,
    ) -> Result<Option<String>, SkillActivationSelectionError> {
        Ok(self
            .messages_by_run
            .lock()
            .map_err(|_| SkillActivationSelectionError::Internal)?
            .remove(&SkillActivationMessageKey::new(
                scope.clone(),
                accepted_message_ref.clone(),
            )))
    }
}

impl Default for SelectableSkillContextSource {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SkillActivationMessageKey {
    scope: TurnScope,
    accepted_message_ref: AcceptedMessageRef,
}

impl SkillActivationMessageKey {
    fn new(scope: TurnScope, accepted_message_ref: AcceptedMessageRef) -> Self {
        Self {
            scope,
            accepted_message_ref,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_host_api::{AgentId, ProjectId, TenantId};
    use brassclaw_turns::{
        AcceptedMessageRef, TurnActor, TurnId, TurnRunId,
        run_profile::{
            InMemoryRunProfileResolver, LoopRunContext, RunProfileResolutionRequest,
            RunProfileResolver,
        },
    };

    async fn run_context_for(thread_id: &str, accepted_message: &str) -> LoopRunContext {
        let resolved = InMemoryRunProfileResolver::default()
            .resolve_run_profile(RunProfileResolutionRequest::interactive_default())
            .await
            .unwrap();
        LoopRunContext::new(
            TurnScope::new(
                TenantId::new("tenant-a").unwrap(),
                Some(AgentId::new("agent-a").unwrap()),
                Some(ProjectId::new("project-a").unwrap()),
                brassclaw_host_api::ThreadId::new(thread_id).unwrap(),
            ),
            TurnId::new(),
            TurnRunId::new(),
            resolved,
        )
        .with_accepted_message_ref(AcceptedMessageRef::new(accepted_message).unwrap())
        .with_actor(TurnActor::new(
            brassclaw_host_api::UserId::new("user-a").unwrap(),
        ))
    }

    fn accepted_message_ref(context: &LoopRunContext) -> AcceptedMessageRef {
        context
            .accepted_message_ref
            .clone()
            .expect("run context accepted message ref")
    }

    #[tokio::test]
    async fn peek_message_text_returns_raw_text_and_is_non_consuming() {
        // v3 plan §H3: `peek_message_text` returns the raw (unsanitized)
        // accepted-message body and is non-consuming — unlike
        // `take_message_for_run`, which removes the entry. This backs the
        // composition `MessageTextResolver` so intent matching reads the raw
        // text (no `[redacted]` placeholders) without destroying the record.
        let source = SelectableSkillContextSource::new();
        let context = run_context_for("thread-a", "msg:run-a").await;
        source
            .record_user_message(
                context.scope.clone(),
                accepted_message_ref(&context),
                "please review this raw PR text",
            )
            .expect("record message");

        // First peek returns the raw recorded text.
        let first_peek = source
            .peek_message_text(&context.scope, &accepted_message_ref(&context))
            .expect("first peek");
        assert_eq!(
            first_peek.as_deref(),
            Some("please review this raw PR text")
        );

        // Non-consuming: a second peek returns the same text (entry still present).
        let second_peek = source
            .peek_message_text(&context.scope, &accepted_message_ref(&context))
            .expect("second peek");
        assert_eq!(
            second_peek.as_deref(),
            Some("please review this raw PR text"),
            "peek must not remove the recorded message"
        );

        // The consuming accessor removes the entry and returns the same raw text.
        let taken = source
            .take_message_for_run(&context.scope, &accepted_message_ref(&context))
            .expect("take message for run")
            .expect("message was recorded");
        assert_eq!(taken, "please review this raw PR text");

        // After the consuming take, peek returns None.
        let peek_after_take = source
            .peek_message_text(&context.scope, &accepted_message_ref(&context))
            .expect("peek after take");
        assert!(
            peek_after_take.is_none(),
            "take must have removed the entry; peek must now miss"
        );
    }

    #[tokio::test]
    async fn peek_message_text_misses_for_unrecorded_ref() {
        // v3 plan §H3: `peek_message_text` returns `None` when no message is
        // recorded for the ref (never written or already taken) — the soft-miss
        // the resolver maps to Tier-2 fall-through.
        let source = SelectableSkillContextSource::new();
        let context = run_context_for("thread-b", "msg:run-b").await;

        let peek = source
            .peek_message_text(&context.scope, &accepted_message_ref(&context))
            .expect("peek on empty store");
        assert!(
            peek.is_none(),
            "peek on a never-written ref must return None"
        );
    }

    #[tokio::test]
    async fn clear_accepted_message_removes_recorded_entry() {
        let source = SelectableSkillContextSource::new();
        let context = run_context_for("thread-c", "msg:run-c").await;
        source
            .record_user_message(
                context.scope.clone(),
                accepted_message_ref(&context),
                "some message",
            )
            .expect("record");
        source
            .clear_accepted_message(&context.scope, &accepted_message_ref(&context))
            .expect("clear");

        let peek = source
            .peek_message_text(&context.scope, &accepted_message_ref(&context))
            .expect("peek after clear");
        assert!(peek.is_none(), "cleared entry must not be visible");
    }
}
