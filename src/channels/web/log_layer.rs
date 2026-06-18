//! Re-exports from `crate::logging` for backward compatibility within `channels::web`.

pub use crate::logging::{
    LogBroadcaster, LogEntry, LogLevelHandle, WebLogLayer, init_tracing,
};

use std::sync::Arc;

use brassclaw_common::DynEventPublisher;

use super::platform::sse::SseManager;

pub fn spawn_warning_bridge(
    broadcaster: Arc<LogBroadcaster>,
    sse: Arc<SseManager>,
    owner_id: Option<String>,
) {
    let ep: DynEventPublisher = sse;
    crate::logging::spawn_warning_bridge(broadcaster, ep, owner_id);
}
