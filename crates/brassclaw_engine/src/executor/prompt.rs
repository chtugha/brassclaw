//! System prompt construction for the execution loop.
//!
//! Builds a CodeAct/RLM system prompt that instructs the LLM to write
//! Python code in ```repl blocks while keeping the model-facing surfaces
//! separated:
//! - callable actions stay in the normal tool inventory
//! - contextual capability background stays in one canonical always-on section
//! - blocked managed integrations are rendered separately under
//!   `Activatable Integrations`
//!
//! The CodeAct preamble (`codeact_preamble`) and postamble (`codeact_postamble`)
//! are seeded as class-10 `reborn_skills` rows by `builtin_bootstrap.rs` and
//! flow to the LLM via the validated prefix bundle assembled by the host
//! prefix-bundle capabilities.
//! They are NOT loaded per-turn by the executor.

// Runtime platform metadata lives in `brassclaw_common`. Re-exported
// from this module's path for back-compat with prior call sites.
pub use brassclaw_common::PlatformInfo;

// All constants, helper functions, and the prompt builder below are
// test-only: the preamble/postamble reach the LLM via the Kohai prefix
// bundle; they are NOT loaded per-turn by the executor.

/// Returns `true` if `title` identifies a protected component that must pass the
/// validation gate (Q1 → Q2) before being applied.
///
/// Protected components:
/// - `orchestrator:main` — the class-10 Python loop driver (self-modifiable).
/// - `codeact_preamble`  — the class-10 CodeAct preamble.
///
/// Any `memory_write` targeting these titles is intercepted by the Rust bridge
/// and routed through `__validate_component__` instead (spec §3.5 / §3.6).
pub fn is_protected_component_title(title: &str) -> bool {
    // orchestrator:main (class 10) and codeact_preamble (class 10) require
    // Q1 LLM audit + Q2 manual validation before any memory_write is applied.
    title == "orchestrator:main" || title == "codeact_preamble"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::capability::{
        ActionDef, CapabilityStatus, CapabilitySummary, CapabilitySummaryKind, ModelToolSurface,
    };
    use crate::types::message::{MessageRole, ThreadMessage};

    /// `CODEACT_PREAMBLE` and `CODEACT_POSTAMBLE` are only needed in tests.
    /// Production code must not use these compiled-in constants: the preamble
    /// and postamble reach the LLM via the Kohai prefix bundle, which queries
    /// class-10 `reborn_skills` rows seeded by `builtin_bootstrap.rs`.
    const CODEACT_PREAMBLE: &str = include_str!("../../prompts/codeact_preamble.md");
    const CODEACT_POSTAMBLE: &str = include_str!("../../prompts/codeact_postamble.md");

    // ── All constants and helpers used only by the test prompt builder ──────

    /// Structured-tools-only preamble used when `BRASSCLAW_DISABLE_CODEACT` is set.
    const STRUCTURED_TOOL_PREAMBLE: &str = r#"You are BrassClaw, a personal AI assistant.

## Execution mode

Use the provider's structured tool_calls interface for every action.
Do not emit Python, repl, py, or other executable fenced code blocks.
Do not call tools as Python functions.
Do not write tool invocations in assistant text. Never output `[[call_tool ...]]`, `<tool_call>`, `<function_call>`, JSON tool-call blobs, or function-style calls such as `tool_name(...)`.
Only the provider-level `tool_calls` field invokes tools. If you need a tool, return a structured tool call instead of describing or printing the call.
When no action is needed, answer in plain text.
"#;

    /// Structured-tools-only postamble used when `BRASSCLAW_DISABLE_CODEACT` is set.
    const STRUCTURED_TOOL_POSTAMBLE: &str = r#"
## Strategy

Use structured tool calls when you need data, persistence, external effects, or system state.
After tool results are available, continue with another structured tool call or return the final plain-text answer.
Some integrations use literal UI blocks such as `[[choice_set]]...[[/choice_set]]` in final user-facing text. These are UI markup only; do not invent other bracketed control blocks, especially `[[call_tool ...]]`.
"#;

    /// Whether CodeAct (Tier 1 Python execution) is disabled by env var.
    fn codeact_disabled() -> bool {
        matches!(
            std::env::var("BRASSCLAW_DISABLE_CODEACT").as_deref(),
            Ok("true" | "1")
        )
    }

    const CODEACT_SYSTEM_PROMPT_MARKER: &str = "<!-- brassclaw:codeact-system-prompt -->\n";
    const CODEACT_LEGACY_OPENING: &str = "You are an AI assistant with a Python REPL environment.";
    const CODEACT_STRATEGY_HEADING: &str = "\n## Strategy\n";
    const CODEACT_CAPABILITIES_HEADING: &str = "\n## Available capabilities (background status)\n";
    const CODEACT_BACKGROUND_CAPABILITIES_HEADING: &str = "\n## Capabilities\n";
    const CODEACT_ENABLED_TOOLS_HEADING: &str = "\n## Enabled Tools\n";
    const CODEACT_ACTIVATABLE_INTEGRATIONS_HEADING: &str = "\n## Activatable Integrations\n";
    const PRIOR_KNOWLEDGE_HEADING: &str = "\n\n## Prior Knowledge (from completed threads)\n";
    const ACTIVE_SKILLS_HEADING: &str = "\n\n## Active Skills\n";
    const MISSING_SKILLS_PREFIX: &str = "\n\nThe user explicitly requested slash skill(s) that are not installed or were not found:";

    const fn capability_status_label(status: CapabilityStatus) -> &'static str {
        match status {
            CapabilityStatus::Ready => "ready",
            CapabilityStatus::ReadyScoped => "ready_scoped",
            CapabilityStatus::NeedsAuth => "needs_auth",
            CapabilityStatus::NeedsSetup => "needs_setup",
            CapabilityStatus::Inactive => "inactive",
            CapabilityStatus::Latent => "latent",
            CapabilityStatus::Error => "error",
            CapabilityStatus::AvailableNotInstalled => "available_not_installed",
        }
    }

    const fn capability_kind_label(kind: CapabilitySummaryKind) -> &'static str {
        match kind {
            CapabilitySummaryKind::Channel => "channel",
            CapabilitySummaryKind::Provider => "provider",
            CapabilitySummaryKind::Runtime => "runtime",
        }
    }

    fn is_activatable_integration(capability: &CapabilitySummary) -> bool {
        matches!(
            capability.kind,
            CapabilitySummaryKind::Provider | CapabilitySummaryKind::Channel
        ) && matches!(
            capability.status,
            CapabilityStatus::NeedsSetup
                | CapabilityStatus::Inactive
                | CapabilityStatus::Latent
                | CapabilityStatus::AvailableNotInstalled
        )
    }

    fn render_background_capability(capability: &CapabilitySummary) -> String {
        let mut line = format!(
            "- `{}` [{}] — {}",
            capability.name,
            capability_kind_label(capability.kind),
            capability_status_label(capability.status)
        );
        if let Some(display_name) = &capability.display_name
            && display_name != &capability.name
        {
            line.push_str(&format!(" ({display_name})"));
        }
        if let Some(routing_hint) = &capability.routing_hint {
            line.push_str(&format!(". {routing_hint}"));
        }
        if let Some(description) = &capability.description {
            line.push_str(&format!(". {description}"));
        }
        line.push('\n');
        line
    }

    fn render_enabled_tool(action: &ActionDef) -> String {
        format!(
            "- `{}` — {}\n",
            action.discovery_name(),
            compact_prompt_description(&action.description)
        )
    }

    fn compact_prompt_description(description: &str) -> String {
        description.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn render_activatable_integration(capability: &CapabilitySummary) -> String {
        let mut line = format!(
            "- `{}` [{}]",
            capability.name,
            capability_kind_label(capability.kind)
        );
        if let Some(display_name) = &capability.display_name
            && display_name != &capability.name
        {
            line.push_str(&format!(" ({display_name})"));
        }
        if let Some(description) = &capability.description {
            line.push_str(&format!(" — {description}"));
        }
        if !capability.action_preview.is_empty() {
            line.push_str(&format!(
                ". Unlocks: {}",
                format_action_preview(&capability.action_preview)
            ));
        }
        line.push('\n');
        line
    }

    fn format_action_preview(actions: &[String]) -> String {
        const MAX_PREVIEW: usize = 3;

        let mut rendered = actions
            .iter()
            .take(MAX_PREVIEW)
            .map(|action| format!("`{action}`"))
            .collect::<Vec<_>>();
        if actions.len() > MAX_PREVIEW {
            rendered.push(format!("+{} more", actions.len() - MAX_PREVIEW));
        }
        rendered.join(", ")
    }

    /// Shared prompt builder for CodeAct/RLM system prompts.
    ///
    /// `disable_codeact` is threaded as an explicit parameter (rather than read
    /// from the env directly) so tests can exercise both branches without
    /// process-global env mutation.
    fn build_codeact_system_prompt_inner(
        disable_codeact: bool,
        capabilities: &[CapabilitySummary],
        compact_actions: &[ActionDef],
        overlay: Option<&str>,
        platform: Option<&PlatformInfo>,
    ) -> String {
        let (preamble, postamble) = if disable_codeact {
            (STRUCTURED_TOOL_PREAMBLE, STRUCTURED_TOOL_POSTAMBLE)
        } else {
            (CODEACT_PREAMBLE, CODEACT_POSTAMBLE)
        };

        let mut prompt = String::from(CODEACT_SYSTEM_PROMPT_MARKER);
        prompt.push_str(preamble);

        if let Some(info) = platform {
            prompt.push_str(&info.to_prompt_section());
        }
        if let Some(overlay) = overlay {
            prompt.push_str("\n\n## Learned Rules (from self-improvement)\n\n");
            prompt.push_str(overlay);
        }

        let (mut activatable_integrations, mut background_capabilities): (Vec<_>, Vec<_>) =
            capabilities
                .iter()
                .partition(|capability| is_activatable_integration(capability));
        background_capabilities.sort_by(|a, b| a.name.cmp(&b.name));
        activatable_integrations.sort_by(|a, b| a.name.cmp(&b.name));

        if !background_capabilities.is_empty() {
            prompt.push_str(CODEACT_BACKGROUND_CAPABILITIES_HEADING);
            prompt.push('\n');
            for capability in background_capabilities {
                prompt.push_str(&render_background_capability(capability));
            }
        }

        if !disable_codeact {
            let mut compact_actions: Vec<&ActionDef> = compact_actions
                .iter()
                .filter(|action| {
                    matches!(action.model_tool_surface, ModelToolSurface::CompactToolInfo)
                })
                .collect();
            compact_actions.sort_by(|a, b| a.name.cmp(&b.name));

            if !compact_actions.is_empty() {
                prompt.push_str(CODEACT_ENABLED_TOOLS_HEADING);
                prompt.push('\n');
                prompt.push_str(
                    "These enabled tools are shown in compact form. Before calling one, always check its schema with `tool_info(name=\"<tool>\", detail=\"schema\")`.\n\n",
                );
                for action in compact_actions {
                    prompt.push_str(&render_enabled_tool(action));
                }
            }
        }

        if !activatable_integrations.is_empty() {
            prompt.push_str(CODEACT_ACTIVATABLE_INTEGRATIONS_HEADING);
            prompt.push('\n');
            prompt.push_str(
                "These integrations need user setup before their tools become callable. \
                 When the user asks to connect/install/enable one of them, call \
                 `tool_install(name=\"<name>\")` directly — don't enumerate alternatives or \
                 describe manual UI steps. If credentials are missing the engine raises an \
                 auth gate at execute time and the user is prompted in chat. \
                 For parameter details before installing, call \
                 `tool_info(name=\"<tool>\", detail=\"summary\")` on a preview tool.\n\n",
            );
            for capability in activatable_integrations {
                prompt.push_str(&render_activatable_integration(capability));
            }
        }

        prompt.push_str(postamble);
        prompt
    }

    fn is_codeact_system_prompt(content: &str) -> bool {
        content.starts_with(CODEACT_SYSTEM_PROMPT_MARKER)
            || is_legacy_codeact_system_prompt(content)
    }

    fn refresh_codeact_system_prompt(existing_content: &str, system_prompt: &str) -> String {
        if !is_codeact_system_prompt(existing_content) {
            return system_prompt.to_string();
        }

        let suffix = codeact_system_prompt_suffix(existing_content).unwrap_or_default();

        if suffix.is_empty() {
            system_prompt.to_string()
        } else {
            let mut refreshed = String::from(system_prompt);
            refreshed.push_str(suffix);
            refreshed
        }
    }

    fn upsert_codeact_system_prompt(
        messages: &mut Vec<ThreadMessage>,
        system_prompt: String,
    ) -> bool {
        if let Some(message) = messages.iter_mut().find(|message| {
            message.role == MessageRole::System && is_codeact_system_prompt(&message.content)
        }) {
            let refreshed = refresh_codeact_system_prompt(&message.content, &system_prompt);
            if message.content == refreshed {
                return false;
            }
            message.content = refreshed;
            return true;
        }

        if messages
            .iter()
            .any(|message| message.role == MessageRole::System)
        {
            return false;
        }

        messages.insert(0, ThreadMessage::system(system_prompt));
        true
    }

    fn is_legacy_codeact_system_prompt(content: &str) -> bool {
        content.starts_with(CODEACT_LEGACY_OPENING)
            && content.contains("```repl")
            && (content.contains(CODEACT_STRATEGY_HEADING)
                || content.contains(CODEACT_CAPABILITIES_HEADING))
    }

    fn codeact_system_prompt_suffix(existing_content: &str) -> Option<&str> {
        let append_markers = [
            PRIOR_KNOWLEDGE_HEADING,
            ACTIVE_SKILLS_HEADING,
            MISSING_SKILLS_PREFIX,
        ];

        let suffix_start = append_markers
            .iter()
            .filter_map(|marker| existing_content.find(marker))
            .min()
            .or_else(|| {
                existing_content
                    .rfind(CODEACT_POSTAMBLE)
                    .map(|idx| idx + CODEACT_POSTAMBLE.len())
            })?;

        existing_content.get(suffix_start..)
    }

    #[test]
    fn prompt_without_store_uses_compiled_preamble() {
        let prompt = build_codeact_system_prompt_inner(false, &[], &[], None, None);
        assert!(prompt.contains("Python REPL environment"));
        assert!(prompt.contains("Strategy"));
        assert!(!prompt.contains("Learned Rules"));
    }

    #[test]
    fn prompt_with_platform_info_injects_identity() {
        let info = PlatformInfo {
            version: Some("1.2.3".into()),
            llm_backend: Some("nearai".into()),
            model_name: Some("qwen3-235b".into()),
            database_backend: Some("libsql".into()),
            active_channels: vec!["telegram".into(), "cli".into()],
            owner_id: Some("alice.near".into()),
            repo_url: Some("https://github.com/chtugha/brassclaw".into()),
        };
        let prompt = build_codeact_system_prompt_inner(false, &[], &[], None, Some(&info));
        assert!(prompt.contains("BrassClaw"));
        assert!(prompt.contains("1.2.3"));
        assert!(prompt.contains("nearai"));
        assert!(prompt.contains("qwen3-235b"));
        assert!(prompt.contains("libsql"));
        assert!(prompt.contains("telegram"));
        assert!(prompt.contains("alice.near"));
        assert!(prompt.contains("github.com/chtugha/brassclaw"));
    }

    #[test]
    fn prompt_without_platform_info_has_no_platform_section() {
        let prompt = build_codeact_system_prompt_inner(false, &[], &[], None, None);
        assert!(!prompt.contains("## Platform"));
    }

    #[test]
    fn prompt_with_capabilities_includes_background_statuses() {
        let prompt = build_codeact_system_prompt_inner(
            false,
            &[
                CapabilitySummary {
                    name: "telegram".into(),
                    display_name: Some("Telegram".into()),
                    kind: crate::types::capability::CapabilitySummaryKind::Channel,
                    status: CapabilityStatus::ReadyScoped,
                    description: Some("Telegram notifications".into()),
                    action_preview: Vec::new(),
                    routing_hint: Some("Usable through message".into()),
                },
                CapabilitySummary {
                    name: "slack".into(),
                    display_name: None,
                    kind: crate::types::capability::CapabilitySummaryKind::Provider,
                    // NeedsSetup (not NeedsAuth) lands in "Activatable
                    // Integrations". NeedsAuth tools are direct-callable
                    // post-#3133, so they live in the regular action
                    // inventory rather than the setup-required section.
                    status: CapabilityStatus::NeedsSetup,
                    description: Some("Slack workspace integration".into()),
                    action_preview: vec!["slack_send".into(), "slack_history".into()],
                    routing_hint: None,
                },
            ],
            &[],
            None,
            None,
        );

        assert!(prompt.contains("## Capabilities"));
        assert!(prompt.contains("`telegram` [channel]"));
        assert!(prompt.contains("ready_scoped"));
        assert!(prompt.contains("Usable through message"));
        assert!(prompt.contains("## Activatable Integrations"));
        assert!(prompt.contains("`slack` [provider]"));
        assert!(prompt.contains("need user setup before their tools become callable"));
        assert!(prompt.contains("tool_info(name=\"<tool>\", detail=\"summary\")"));
        assert!(prompt.contains("Unlocks: `slack_send`, `slack_history`"));
        // Regression for #3533: the prompt must direct the model to call
        // tool_install for activatable integrations instead of narrating
        // manual UI steps or enumerating alternatives.
        assert!(prompt.contains("tool_install(name=\"<name>\")"));
        assert!(prompt.contains("don't enumerate alternatives"));
    }

    /// Prefix-cache invariant: capabilities rendered into the same section
    /// of the system prompt must appear in alphabetical order regardless
    /// of input order, so the stable prefix-cache hit rate is preserved
    /// across turns.
    ///
    /// Sections themselves are emitted in a fixed DOM order by the
    /// renderer (Background Capabilities → Enabled Tools → Activatable
    /// Integrations), so cross-section alphabetic comparisons are NOT a
    /// valid invariant — items in earlier sections are guaranteed to
    /// appear before items in later sections by section semantics, not by
    /// the sort. The within-section sort is what protects per-section
    /// prefix-cache stability.
    ///
    /// `discord` and `telegram` are both background channel capabilities.
    /// The test input places `telegram` BEFORE `discord` (reverse
    /// alphabetical within the Background section), so a regression in
    /// the sort guard would mirror the input order. The assertion below
    /// verifies the rendered order matches the alphabetic invariant.
    #[test]
    fn prompt_capabilities_are_alphabetic_for_prefix_cache_stability() {
        let prompt = build_codeact_system_prompt_inner(
            false,
            &[
                // Background capability, listed FIRST but sorted AFTER
                // `discord` because 'd' < 't'.
                CapabilitySummary {
                    name: "telegram".into(),
                    display_name: Some("Telegram".into()),
                    kind: CapabilitySummaryKind::Channel,
                    status: CapabilityStatus::ReadyScoped,
                    description: Some("Telegram notifications".into()),
                    action_preview: Vec::new(),
                    routing_hint: None,
                },
                // Background capability, listed SECOND but sorted BEFORE
                // `telegram` because 'd' < 't'.
                CapabilitySummary {
                    name: "discord".into(),
                    display_name: Some("Discord".into()),
                    kind: CapabilitySummaryKind::Channel,
                    status: CapabilityStatus::ReadyScoped,
                    description: Some("Discord notifications".into()),
                    action_preview: Vec::new(),
                    routing_hint: None,
                },
            ],
            &[],
            None,
            None,
        );

        let discord_pos = prompt
            .find("- `discord`")
            .expect("discord must be rendered in Background Capabilities section");
        let telegram_pos = prompt
            .find("- `telegram`")
            .expect("telegram must be rendered in Background Capabilities section");
        assert!(
            discord_pos < telegram_pos,
            "background capabilities must be rendered in alphabetical order \
             (discord before telegram) for prefix-cache stability, but the \
             rendered prompt places discord at {discord_pos} and telegram \
             at {telegram_pos}"
        );
    }

    /// Prefix-cache invariant: compact actions rendered into the Enabled
    /// Tools section must appear in alphabetical order regardless of input
    /// order. The listing is part of the stable prefix, so any reordering
    /// between turns invalidates the cache.
    ///
    /// `weather` (compact) is alphabetically AFTER `mission_create`
    /// (compact) but the test input places them in the reverse order. If
    /// the sort guard regresses, the rendered Enabled Tools list mirrors
    /// the input order and the assertion below flips.
    #[test]
    fn prompt_compact_actions_are_alphabetic_for_prefix_cache_stability() {
        let actions = vec![
            // Listed FIRST but sorted AFTER `mission_create` because 'm' < 'w'.
            ActionDef {
                name: "weather".into(),
                description: "Weather lookup.".into(),
                parameters_schema: serde_json::json!({"type": "object"}),
                effects: Vec::new(),
                requires_approval: false,
                model_tool_surface: ModelToolSurface::CompactToolInfo,
                discovery: None,
            },
            // Listed SECOND but sorted BEFORE `weather` because 'm' < 'w'.
            ActionDef {
                name: "mission_create".into(),
                description: "Create scheduled or event-driven missions.".into(),
                parameters_schema: serde_json::json!({"type": "object"}),
                effects: Vec::new(),
                requires_approval: false,
                model_tool_surface: ModelToolSurface::CompactToolInfo,
                discovery: None,
            },
        ];
        let prompt = build_codeact_system_prompt_inner(false, &[], &actions, None, None);

        let mission_pos = prompt
            .find("- `mission_create`")
            .expect("mission_create must be rendered in Enabled Tools section");
        let weather_pos = prompt
            .find("- `weather`")
            .expect("weather must be rendered in Enabled Tools section");
        assert!(
            mission_pos < weather_pos,
            "compact actions must be rendered in alphabetical order in the \
             Enabled Tools section (mission_create before weather) for \
             prefix-cache stability, but the rendered prompt places \
             mission_create at {mission_pos} and weather at {weather_pos}"
        );
    }

    #[test]
    fn prompt_renders_compact_enabled_tools_once_with_schema_instruction() {
        let prompt = build_codeact_system_prompt_inner(
            false,
            &[CapabilitySummary {
                name: "gmail".into(),
                display_name: Some("Gmail".into()),
                kind: CapabilitySummaryKind::Provider,
                // NeedsSetup keeps gmail in Activatable Integrations.
                // NeedsAuth gmail would render in the regular action
                // inventory instead (post-#3133 direct-callable path).
                status: CapabilityStatus::NeedsSetup,
                description: Some("Gmail integration".into()),
                action_preview: vec!["gmail_send".into()],
                routing_hint: None,
            }],
            &[
                ActionDef {
                    name: "mission_create".into(),
                    description: "Create scheduled or event-driven missions.".into(),
                    parameters_schema: serde_json::json!({"type": "object"}),
                    effects: Vec::new(),
                    requires_approval: false,
                    model_tool_surface: ModelToolSurface::CompactToolInfo,
                    discovery: None,
                },
                ActionDef {
                    name: "http".into(),
                    description: "Make HTTP requests.".into(),
                    parameters_schema: serde_json::json!({"type": "object"}),
                    effects: Vec::new(),
                    requires_approval: false,
                    model_tool_surface: ModelToolSurface::FullSchema,
                    discovery: None,
                },
            ],
            None,
            None,
        );

        assert!(prompt.contains("## Enabled Tools"));
        assert_eq!(prompt.matches("## Enabled Tools").count(), 1);
        assert!(prompt.contains(
            "Before calling one, always check its schema with `tool_info(name=\"<tool>\", detail=\"schema\")`."
        ));
        assert!(prompt.contains("- `mission_create`"));
        assert!(!prompt.contains("mission_create(name, goal, cadence"));
        assert!(!prompt.contains("- `http`"));
        assert!(prompt.contains("## Activatable Integrations"));
        assert_eq!(prompt.matches("`gmail` [provider]").count(), 1);
    }

    #[test]
    fn needs_auth_capability_is_not_activatable_integration() {
        // Post-#3133: gmail with NeedsAuth status (installed but missing
        // OAuth) is direct-callable. The auth gate raises at execute
        // time, so the capability does NOT belong in the Activatable
        // Integrations section.
        let prompt = build_codeact_system_prompt_inner(
            false,
            &[CapabilitySummary {
                name: "gmail".into(),
                display_name: Some("Gmail".into()),
                kind: CapabilitySummaryKind::Provider,
                status: CapabilityStatus::NeedsAuth,
                description: Some("Gmail integration".into()),
                action_preview: vec!["gmail_send".into()],
                routing_hint: None,
            }],
            &[],
            None,
            None,
        );
        assert!(!prompt.contains("## Activatable Integrations"));
    }

    #[test]
    fn prompt_no_longer_duplicates_callable_tool_inventory() {
        let prompt = build_codeact_system_prompt_inner(false, &[], &[], None, None);

        assert!(!prompt.contains("## Available tools (call as Python functions)"));
        assert!(!prompt.contains("`message(text)`"));
    }

    #[test]
    fn prompt_keeps_callable_tools_out_of_extra_prompt_sections() {
        let prompt = build_codeact_system_prompt_inner(false, &[], &[], None, None);

        assert!(!prompt.contains("## Lookup-only tools"));
        assert!(!prompt.contains("## Deferred large tools"));
        assert!(!prompt.contains("inspect one on demand"));
        assert!(!prompt.contains("oneOf"));
        assert!(!prompt.contains("\"query\""));
    }

    #[test]
    fn upsert_replaces_engine_owned_system_prompt() {
        let old_prompt = build_codeact_system_prompt_inner(false, &[], &[], None, None);
        let new_prompt = build_codeact_system_prompt_inner(
            false,
            &[CapabilitySummary {
                name: "telegram".into(),
                display_name: None,
                kind: CapabilitySummaryKind::Channel,
                status: CapabilityStatus::ReadyScoped,
                description: None,
                action_preview: Vec::new(),
                routing_hint: Some("Usable through message".into()),
            }],
            &[],
            None,
            None,
        );
        let mut messages = vec![ThreadMessage::system(old_prompt), ThreadMessage::user("hi")];

        assert!(upsert_codeact_system_prompt(
            &mut messages,
            new_prompt.clone()
        ));
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[0].content, new_prompt);
    }

    #[test]
    fn refresh_preserves_step_zero_system_appends() {
        let old_prompt = build_codeact_system_prompt_inner(false, &[], &[], None, None);
        let existing = format!(
            "{old_prompt}\n\n## Prior Knowledge (from completed threads)\n\n### [LESSON] Use http\n\n<skill name=\"github\" version=\"1\">\nGitHub API Skill\n</skill>\n\nThe user explicitly requested slash skill(s) that are not installed."
        );
        let new_prompt = build_codeact_system_prompt_inner(
            false,
            &[CapabilitySummary {
                name: "slack".into(),
                display_name: None,
                kind: CapabilitySummaryKind::Provider,
                status: CapabilityStatus::NeedsAuth,
                description: None,
                action_preview: vec!["slack_send".into()],
                routing_hint: None,
            }],
            &[],
            None,
            None,
        );

        let refreshed = refresh_codeact_system_prompt(&existing, &new_prompt);
        assert!(refreshed.starts_with(&new_prompt));
        assert!(refreshed.contains("## Prior Knowledge (from completed threads)"));
        assert!(refreshed.contains("GitHub API Skill"));
        assert!(refreshed.contains("slash skill(s) that are not installed"));
    }

    #[test]
    fn upsert_replaces_legacy_codeact_prompt_revisions() {
        let legacy_prompt = format!(
            "{CODEACT_LEGACY_OPENING}\n\nLegacy prompt body.\n\n```repl\nprint('hi')\n```\n{CODEACT_STRATEGY_HEADING}\nLegacy strategy text.\n"
        );
        let new_prompt = build_codeact_system_prompt_inner(false, &[], &[], None, None);
        let mut messages = vec![
            ThreadMessage::system(legacy_prompt),
            ThreadMessage::user("resume me"),
        ];

        assert!(upsert_codeact_system_prompt(
            &mut messages,
            new_prompt.clone()
        ));
        assert_eq!(messages[0].content, new_prompt);
    }

    #[test]
    fn refresh_preserves_appends_for_legacy_prompt_revisions() {
        let legacy_prompt = format!(
            "{CODEACT_LEGACY_OPENING}\n\nLegacy prompt body.\n\n```repl\nprint('hi')\n```\n{CODEACT_STRATEGY_HEADING}\nLegacy strategy text.\n{PRIOR_KNOWLEDGE_HEADING}\n### [LESSON] Use http\n\n## Active Skills\n\n<skill name=\"github\" version=\"1\">\nGitHub API Skill\n</skill>\n\nThe user explicitly requested slash skill(s) that are not installed or were not found: /missing."
        );
        let new_prompt = build_codeact_system_prompt_inner(
            false,
            &[CapabilitySummary {
                name: "slack".into(),
                display_name: None,
                kind: CapabilitySummaryKind::Provider,
                status: CapabilityStatus::NeedsAuth,
                description: None,
                action_preview: vec!["slack_send".into()],
                routing_hint: None,
            }],
            &[],
            None,
            None,
        );

        let refreshed = refresh_codeact_system_prompt(&legacy_prompt, &new_prompt);
        assert!(refreshed.starts_with(&new_prompt));
        assert!(refreshed.contains("## Prior Knowledge (from completed threads)"));
        assert!(refreshed.contains("GitHub API Skill"));
        assert!(refreshed.contains("/missing"));
    }

    /// PR #3665 review (serrrfirat). With CodeAct disabled the structured-tool
    /// prompt previously listed compact actions under "## Enabled Tools" with
    /// a `tool_info` schema-lookup instruction — but the LLM adapter only
    /// emitted FullSchema actions to the provider tool list. The result was
    /// that compact tools (mission_create, gmail_send, notion_search, ...)
    /// appeared in the prompt as "available" but could not be called via
    /// `tool_calls`. Fix: skip the "Enabled Tools" section in disabled mode
    /// and emit every action into the provider tool list instead (the
    /// adapter-side half of this fix lives in `src/bridge/llm_adapter.rs`).
    #[test]
    fn disabled_codeact_omits_enabled_tools_section_and_keeps_activatable() {
        let actions = vec![
            ActionDef {
                name: "mission_create".into(),
                description: "Create scheduled or event-driven missions.".into(),
                parameters_schema: serde_json::json!({"type": "object"}),
                effects: Vec::new(),
                requires_approval: false,
                model_tool_surface: ModelToolSurface::CompactToolInfo,
                discovery: None,
            },
            ActionDef {
                name: "http".into(),
                description: "Make HTTP requests.".into(),
                parameters_schema: serde_json::json!({"type": "object"}),
                effects: Vec::new(),
                requires_approval: false,
                model_tool_surface: ModelToolSurface::FullSchema,
                discovery: None,
            },
        ];
        let capabilities = vec![CapabilitySummary {
            name: "gmail".into(),
            display_name: Some("Gmail".into()),
            kind: CapabilitySummaryKind::Provider,
            status: CapabilityStatus::NeedsSetup,
            description: Some("Gmail integration".into()),
            action_preview: vec!["gmail_send".into()],
            routing_hint: None,
        }];

        // Enabled (control): the existing section renders.
        let enabled = build_codeact_system_prompt_inner(false, &capabilities, &actions, None, None);
        assert!(enabled.contains("## Enabled Tools"));
        assert!(enabled.contains("- `mission_create`"));
        assert!(enabled.contains("## Activatable Integrations"));

        // Disabled: section is gone, but Activatable Integrations stays
        // (the model still needs to know what `tool_install` can target),
        // and `mission_create` does NOT appear in the prompt — it's only
        // reachable via the provider tool list now.
        let disabled = build_codeact_system_prompt_inner(true, &capabilities, &actions, None, None);
        assert!(
            !disabled.contains("## Enabled Tools"),
            "Enabled Tools section must be omitted in disabled-CodeAct mode"
        );
        assert!(
            !disabled.contains("mission_create"),
            "compact action must not appear in prompt — it's in the provider tool list"
        );
        assert!(
            !disabled.contains("detail=\"schema\""),
            "schema-lookup instruction is meaningless when provider sends full schemas \
             (the `detail=\"summary\"` reference in Activatable Integrations is fine)"
        );
        assert!(
            disabled.contains("## Activatable Integrations"),
            "Activatable Integrations is still needed so the model can tool_install"
        );
    }
}
