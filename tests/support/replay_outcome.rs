//! Snapshot-oriented view of what a replay produced.
//!
//! Reviewers diff the snapshot, not the raw JSON. Keep the struct narrow so
//! small prompt-wording changes don't force snapshot churn.

#![allow(dead_code)] // Consumed by snapshot tests gated by features.
#![allow(unreachable_pub)] // Integration test support — items pub for use across test modules.

use std::collections::BTreeMap;

use serde::Serialize;

use brassclaw::channels::{OutgoingResponse, StatusUpdate};

/// Minimal introspection surface that `ReplayOutcome::capture` needs.
///
/// Implement this on any V2 harness that wants snapshot-based regression
/// coverage.
pub trait ReplayRig {
    fn captured_status_events(&self) -> Vec<StatusUpdate>;
    fn tool_calls_completed(&self) -> Vec<(String, bool)>;
    fn llm_call_count(&self) -> u32;
}

/// Short summary of a single status event.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventSummary {
    Thinking {
        message: String,
    },
    ToolStarted {
        name: String,
    },
    ToolCompleted {
        name: String,
        success: bool,
        error: Option<String>,
    },
    ToolResultPreview {
        name: String,
        preview_len_bucket: usize,
    },
    Status {
        message: String,
    },
    ApprovalNeeded {
        tool_name: String,
    },
    AuthRequired {
        extension_name: String,
    },
    AuthCompleted {
        extension_name: String,
        success: bool,
    },
    Suggestions {
        count: usize,
    },
    Other {
        variant: &'static str,
    },
}

/// Summary of a single tool invocation the rig observed.
#[derive(Debug, Serialize)]
pub struct ToolCallSummary {
    pub name: String,
    pub success: bool,
}

/// Top-level snapshot of a completed replay run.
#[derive(Debug, Serialize)]
pub struct ReplayOutcome {
    pub response_count: usize,
    pub has_final_response: bool,
    pub tool_calls: Vec<ToolCallSummary>,
    pub events: Vec<EventSummary>,
    pub event_kind_counts: BTreeMap<String, usize>,
    pub llm_call_count: u32,
    pub safety_warning_count: usize,
}

impl ReplayOutcome {
    /// Capture the outcome of a just-completed replay.
    pub async fn capture(rig: &impl ReplayRig, responses: &[OutgoingResponse]) -> Self {
        let status_events = rig.captured_status_events();
        let mut events = Vec::with_capacity(status_events.len());
        let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut safety_warning_count = 0usize;

        for event in status_events {
            let summary = match event {
                StatusUpdate::Thinking(msg) => {
                    *kind_counts.entry("Thinking".into()).or_default() += 1;
                    EventSummary::Thinking {
                        message: bucket_text(&msg),
                    }
                }
                StatusUpdate::ToolStarted { name, .. } => {
                    *kind_counts.entry("ToolStarted".into()).or_default() += 1;
                    EventSummary::ToolStarted {
                        name: strip_tool_params(&name),
                    }
                }
                StatusUpdate::ToolCompleted {
                    name,
                    success,
                    error,
                    ..
                } => {
                    *kind_counts.entry("ToolCompleted".into()).or_default() += 1;
                    EventSummary::ToolCompleted {
                        name: strip_tool_params(&name),
                        success,
                        error: error.map(|e| bucket_text(&e)),
                    }
                }
                StatusUpdate::ToolResult { name, preview, .. } => {
                    *kind_counts.entry("ToolResult".into()).or_default() += 1;
                    EventSummary::ToolResultPreview {
                        name: strip_tool_params(&name),
                        preview_len_bucket: bucket_usize(preview.chars().count(), 100),
                    }
                }
                StatusUpdate::StreamChunk(_) => {
                    *kind_counts.entry("StreamChunk".into()).or_default() += 1;
                    continue;
                }
                StatusUpdate::Status(msg) => {
                    *kind_counts.entry("Status".into()).or_default() += 1;
                    if is_safety_warning(&msg) {
                        safety_warning_count += 1;
                    }
                    EventSummary::Status {
                        message: bucket_text(&msg),
                    }
                }
                StatusUpdate::ApprovalNeeded { tool_name, .. } => {
                    *kind_counts.entry("ApprovalNeeded".into()).or_default() += 1;
                    EventSummary::ApprovalNeeded { tool_name }
                }
                StatusUpdate::AuthRequired { extension_name, .. } => {
                    *kind_counts.entry("AuthRequired".into()).or_default() += 1;
                    EventSummary::AuthRequired {
                        extension_name: extension_name.into(),
                    }
                }
                StatusUpdate::AuthCompleted {
                    extension_name,
                    success,
                    ..
                } => {
                    *kind_counts.entry("AuthCompleted".into()).or_default() += 1;
                    EventSummary::AuthCompleted {
                        extension_name: extension_name.into(),
                        success,
                    }
                }
                StatusUpdate::Suggestions { suggestions } => {
                    *kind_counts.entry("Suggestions".into()).or_default() += 1;
                    EventSummary::Suggestions {
                        count: suggestions.len(),
                    }
                }
                _ => {
                    *kind_counts.entry("Other".into()).or_default() += 1;
                    EventSummary::Other { variant: "Other" }
                }
            };
            events.push(summary);
        }

        let tool_calls = rig
            .tool_calls_completed()
            .into_iter()
            .map(|(name, success)| ToolCallSummary {
                name: strip_tool_params(&name),
                success,
            })
            .collect();

        Self {
            response_count: responses.len(),
            has_final_response: !responses.is_empty(),
            tool_calls,
            events,
            event_kind_counts: kind_counts,
            llm_call_count: rig.llm_call_count(),
            safety_warning_count,
        }
    }
}

fn strip_tool_params(name: &str) -> String {
    match name.find('(') {
        Some(i) => name[..i].to_string(),
        None => name.to_string(),
    }
}

fn bucket_text(s: &str) -> String {
    let trimmed: String = s.chars().take(40).collect();
    trimmed.to_lowercase()
}

fn bucket_usize(value: usize, bucket: usize) -> usize {
    if bucket == 0 {
        return value;
    }
    (value / bucket) * bucket
}

fn is_safety_warning(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("sanitiz") || lower.contains("inject") || lower.contains("warning")
}

/// Assert that `outcome` matches the saved YAML snapshot for `name`.
#[macro_export]
macro_rules! assert_replay_snapshot {
    ($name:expr, $outcome:expr) => {{
        let mut settings = ::insta::Settings::clone_current();
        settings.set_snapshot_path(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots"));
        settings.set_prepend_module_to_snapshot(false);
        settings.set_sort_maps(true);
        settings.set_omit_expression(true);
        settings.bind(|| {
            ::insta::assert_yaml_snapshot!(format!("replay__{}", $name), $outcome);
        });
    }};
}
